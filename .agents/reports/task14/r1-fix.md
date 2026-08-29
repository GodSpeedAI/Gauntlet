# R-1 fix: `$WORKSPACE_ROOT` scoping in `PolicyFs::policy_for`

Status: DONE — all gates green.

Files touched (exactly the ones the task named, nothing else):
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` (+234/-15... net
  diff; the fix, doc comments, 3 new tests, 5 existing tests updated)
- `crates/gauntlet-cli/tests/p5_hard_gate.rs` (+55/-9 net diff; two
  directory-creation fixes, two new named constants, one in-sandbox
  write-target correction, one denylist-safe comment reword)
- `.agents/reports/task14/r1-fix.md` (this checkpoint report, new)

No other file was read-modified. `gate.rs`, `controller.rs`, and
everything under `crates/gauntlet-app/src/control/` were left untouched
(R-1b, out of scope — see below). `.gauntlet/policies/*.json` content,
`.wayfinder/**`, and `.agents/CURRENT_STATUS.yml` were not touched.

## The defect (confirmed by direct source reading)

`crates/gauntlet-ports/src/policy_provider.rs` declares
`policy_for(&self, tier, workspace: &WorkspaceRef, deadline)`. The
implementation in `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs`
(`impl PolicyProvider for PolicyFs`, `policy_for`, was at line 389) bound
the parameter as `_workspace: &WorkspaceRef` — read never — and called
`Self::decode(tier, &file, &doc, &self.anchors)` with the anchors fixed at
`PolicyFs::new` construction time. `resolve_anchored_path` then resolves
`$WORKSPACE_ROOT` straight to `anchors.workspace_root`, the doc-commented
"root under which run workspaces live" — the SHARED PARENT of every run's
and every unit's workspace, not any one workspace. Every builder dispatched
with the shipped `.gauntlet/policies/*.json` (`"writable_roots":
["$WORKSPACE_ROOT"]`) was granted read-write over every OTHER unit's and
run's workspace too.

## The fix

`$WORKSPACE_ROOT` in a policy entry now means "the caller's OWN workspace
for THIS `policy_for` call": `anchors.workspace_root.join(<validated
workspace segment>)`, never the bare shared parent.

1. New `validated_workspace_segment(&WorkspaceRef) -> Result<&str,
   PolicyProviderError>` — requires non-empty, every byte in
   `[A-Za-z0-9._-]`, and not exactly `.` or `..`. Anything else is a typed
   `Invalid` naming the offending workspace ref. `WorkspaceRef::new`'s own
   validation (`gauntlet-domain/src/values.rs::validate_plain_ref`) does
   NOT reject `..`, `.`, or embedded `/`, so this closes the same class of
   anchor-escape bug COR-2 closed for policy-entry paths, applied to the
   workspace segment.
2. `policy_for` builds an owned, per-call `AnchorRoots` with
   `workspace_root: self.anchors.workspace_root.join(validated_segment)`,
   `project_root`/`home` unchanged, and passes `&effective_anchors` into
   `decode` instead of `&self.anchors`.
3. `resolve_anchored_path`/`contain_within_anchor` are UNCHANGED — they
   already implement spec §17.5.2 correctly (COR-2/COR-5). Passing the
   narrower per-call anchor through the existing plumbing is enough, and
   makes `$WORKSPACE_ROOT/../sibling-key` now correctly refused (escapes
   the new narrower anchor, though it stayed inside the old shared-parent
   anchor) — a genuine containment improvement, demonstrated by a new test.

## Plan (checkpoints below, updated as each completes)

- [x] Step 1+2: validator (`validated_workspace_segment`) + per-call anchor
      scoping in `gauntlet-adapter-policy-fs/src/lib.rs`'s `policy_for`.
      Order: irreversible check -> workspace segment validation (pure,
      no IO) -> per-call `effective_anchors` built -> deadline check ->
      file read -> `decode(tier, &file, &doc, &effective_anchors)`.
- [x] Step 3: doc comment updates (module header ~16-23/24-34 now, `AnchorRoots.
      workspace_root` field doc, `resolve_anchored_path` function doc) — all
      three now state $WORKSPACE_ROOT resolves to the per-call scoped
      subdirectory, never the bare shared parent injected at construction.
- [x] Step 4: regression tests written FIRST, confirmed RED for the right
      reason against the un-fixed code (see teeth section), then GREEN
      after the fix. All 3 new tests + every pre-existing
      `$WORKSPACE_ROOT`-using test surveyed and updated (see "test survey"
      below). `cargo nextest run -p gauntlet-adapter-policy-fs`: 20/20 PASS.
- [x] Step 5: `p5_hard_gate.rs` fixture updates — done. `live_govern`
      creates `workspace.join(GATE_WORKSPACE_REF)` before returning (fixes
      all 4 call sites uniformly); the shipped-artifacts tail of
      `containment_wraps_from_process_start_and_confines_the_session`
      creates `workspace.join(SHIPPED_WORKSPACE_REF)` before its
      `policy_for`/`validate` loop. That same test's REAL in-sandbox write
      assertions were retargeted from bare `workspace` to
      `workspace.join(GATE_WORKSPACE_REF)` (the fix narrows the actual
      writable root the policy grants — see "beyond the two named call
      sites" note below). `cargo nextest run -p gauntlet-cli --test
      p5_hard_gate`: 4/4 PASS, including the real bwrap sandbox execution
      leg (0.158s — the backend is present on this host and the live legs
      genuinely ran, not skipped).
- [x] Step 6: teeth (sha256 before/after, RED/GREEN legs) — see below.
- [x] Gates: ALL GREEN — see "Final gate results" below.

## Mid-course correction found by the gates (recorded honestly)

`just boundaries` caught something my own edits introduced: a prose
comment I wrote in `p5_hard_gate.rs` ("...bare `workspace` is now only
read-only-visible (the bwrap adapter's baseline `--ro-bind / /`)...")
named the sandbox product directly. `gauntlet-arch-tests`'s denylist (spec
§24.3, `crates/gauntlet-arch-tests/src/denylist.rs::DENIED_TERMS`) forbids
vendor/product identifiers like `bwrap` in prose outside
`crates/adapters/**` — this crate (`gauntlet-cli`) is a core crate, not an
adapter. The two flagged lines were MY OWN new comment, not pre-existing
code (confirmed: `gauntlet_adapter_sandbox_bwrap::locate()` crate-path
usages elsewhere in the same file were NOT flagged — the denylist exempts
real crate-path identifiers, only prose mentions are denied). Fixed by
rewording to "the real backend's own default containment posture"
(matching the file's own established discipline elsewhere: "the REAL
adapter's own identity, derived at runtime inside `live_govern` — never
spelled in this crate (the boundary suite denies it)"). Re-ran
`gauntlet-arch-tests`: 215/215 PASS. Re-ran `cargo fmt --all -- --check`
and the `p5_hard_gate`/`g7_tier_downgrade` suite again after the reword:
still clean, still 7/7 PASS.

## Final gate results (run in the order the task specifies, sequentially,
one cargo/nextest invocation at a time, per BUILD-LOCK)

1. `just check` (`cargo check --workspace --all-targets`): **PASS** —
   `Finished dev profile [unoptimized] target(s) in 5.28s`, exit 0.
2. `cargo nextest run -p gauntlet-adapter-policy-fs`: **20/20 PASS, 0
   skipped.**
3. `cargo nextest run -p gauntlet-app govern:: doctor::`: **44/44 PASS,
   302 skipped** (skipped = every other `gauntlet-app` test outside those
   two filters, expected).
4. `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test
   g7_tier_downgrade`: **7/7 PASS, 0 skipped** — including the real
   sandboxed-execution leg
   `containment_wraps_from_process_start_and_confines_the_session`
   (~0.13s; the process-isolation backend is present on this host, so
   this ran for real, not skipped).
5. `just lint`:
   - `cargo clippy --workspace --all-targets -- -D warnings`: **PASS**,
     zero warnings, exit 0.
   - `cargo fmt --check`: **PASS** after `cargo fmt --all` was applied
     once to the two touched files (pure whitespace/wrapping, no
     semantic change — re-verified both affected test suites green
     afterward).
6. `just boundaries` (`cargo nextest run -p gauntlet-arch-tests`):
   **215/215 PASS, 0 skipped** (first run caught the denylist
   regression above; fixed; re-run clean).
7. `just doctor`: **PASS**, `exit: OK`, report at
   `.agents/reports/doctor.json`. All 8 preflight checks pass or report
   their honest unavailable/skipped state (runner resolution and runner
   capabilities are `unavailable` — expected, plan Tasks 15-16 scope,
   unrelated to this fix).
8. `just ruler` (`./scripts/verify-ruler.sh`): **PASS** — "answer key and
   manifest digests match the frozen lock."

No test was weakened, skipped, or deleted to reach green. The one defect
the gates caught (the `bwrap` prose mention) was diagnosed and fixed, not
silenced.

## Step 6 teeth evidence

Fixed-file baseline sha256 (`crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs`):
`a6a682285350d673a21eb2220d356dc8ad3cb25fc8337406d287fa0f9814be1b`

### Leg 1 — revert `policy_for`'s `decode` call to `&self.anchors` (unscoped)

Mutation: `Self::decode(tier, &file, &doc, &effective_anchors)` ->
`Self::decode(tier, &file, &doc, &self.anchors)` (one line).

`cargo nextest run -p gauntlet-adapter-policy-fs
two_different_workspaces_resolve_to_disjoint_writable_roots
a_relative_escape_through_the_workspace_segment_is_refused`:

- `two_different_workspaces_resolve_to_disjoint_writable_roots`: **FAILED**
  — `assertion `left != right` failed ... left: ["/run/workspaces"] right:
  ["/run/workspaces"]`. RED for the right reason: both distinct
  `WorkspaceRef`s resolved to the identical bare shared parent — exactly
  R-1's original defect, exactly what this test exists to catch.
- `a_relative_escape_through_the_workspace_segment_is_refused`: **still
  PASSED** even under this mutation. This is the honestly-reported,
  empirically-confirmed fact behind the "on the escape test's actual
  teeth" note above: `contain_within_anchor`'s check is symmetric in the
  anchor's absolute depth — a `..`-shaped escape off `$WORKSPACE_ROOT`
  escapes whatever `$WORKSPACE_ROOT` currently names, bare or scoped —
  so reverting the scoping alone does not flip this specific test. It
  remains a true, meaningful pin (COR-2's containment check now runs
  against the correct, narrower anchor), just not one that this
  particular mutation falsifies.

Restore: byte-identical (verified — sha256 back to
`a6a68228...4be1b` exactly). Re-run: both tests **PASS**; full package
re-run: 20/20 PASS.

### Leg 2 — disable the validator (`let sanitized = true;`)

Mutation: `validated_workspace_segment`'s `sanitized` computation replaced
with the literal `true` (one line).

`cargo nextest run -p gauntlet-adapter-policy-fs an_unsanitized_workspace_ref_is_refused`:

- **FAILED** — `an unsanitized workspace ref ".." must be Invalid, got
  Ok(SandboxPolicy { ... writable_roots: ["/run"], ... })`. RED for the
  right reason, and it demonstrates WHY Step 1 (the validator) is a
  prerequisite for Step 2 (the scoping) to be safe: with sanitization
  disabled, a `WorkspaceRef` of `".."` joined onto
  `anchors.workspace_root` (`/run/workspaces`) resolves to `/run` —
  literally walking the scoped anchor back UP to the shared parent's own
  parent, a fresh anchor-escape reopened one level up from the one COR-2
  already closed for policy-entry paths.

Restore: byte-identical (verified — sha256 back to
`a6a68228...4be1b` exactly). Re-run: full package: 20/20 PASS.

## Test survey: every pre-existing test using a `$WORKSPACE_ROOT`-anchored
entry together with a `WorkspaceRef` (`grep -n 'WORKSPACE_ROOT'` first, per
the task instruction), and what changed

All of these use the fixed `workspace_ref()` fixture, which returns
`WorkspaceRef::new("ws-policy-fs-test")` — so after the fix, every
`$WORKSPACE_ROOT` in these fixtures resolves to
`anchors().workspace_root.join("ws-policy-fs-test")` =
`/run/workspaces/ws-policy-fs-test` (test-double anchors) instead of bare
`/run/workspaces`.

- `every_non_irreversible_tier_resolves_from_its_artifact`: `contained.
  writable_roots` and `cross.writable_roots`' first entry updated from
  `"/run/workspaces"` to `"/run/workspaces/ws-policy-fs-test"`. PURPOSE
  UNCHANGED (still proves the artifact decodes verbatim per tier) — only
  the expected literal changed to reflect correct scoping.
- `the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier`:
  expected `contained.writable_roots` gained a `.join("ws-policy-fs-test")`
  segment. PURPOSE UNCHANGED.
- `an_anchored_path_may_not_escape_its_anchor`,
  `an_anchor_sibling_sharing_a_byte_prefix_is_not_contained`: NO CHANGE
  needed — both assert `Err(Invalid)` + `detail.contains("escapes")` only,
  no literal resolved-path value is asserted, and (verified by direct
  computation, confirmed empirically by the unchanged pass) a `..`-based
  escape off `$WORKSPACE_ROOT` was ALREADY refused pre-fix and remains
  refused post-fix — see "on the escape test's actual teeth" below for why
  this is expected and not a gap.
- `interior_dot_segments_normalize_inside_the_anchor`: expected
  `"/run/workspaces/b/c"` -> `"/run/workspaces/ws-policy-fs-test/b/c"`.
  PURPOSE UNCHANGED (still proves interior `.`/`..` normalize rather than
  refuse).
- `a_path_that_does_not_exist_yet_still_resolves`: expected
  `"/run/workspaces/not/created/yet"` ->
  `"/run/workspaces/ws-policy-fs-test/not/created/yet"`. PURPOSE UNCHANGED.
- `a_symlink_out_of_the_anchor_is_an_escape`: RESTRUCTURED, not just a
  literal update — the symlink and the "real" directory now live under
  `workspace_root.join(workspace_ref().as_str())` (the effective anchor)
  instead of bare `workspace_root`, since that bare path is no longer
  where `$WORKSPACE_ROOT` resolves. PURPOSE UNCHANGED (still proves a
  symlink escape through the anchor is caught, and a real in-anchor path
  still resolves) but the fixture's own directory layout had to move down
  one level to stay meaningful — flagged explicitly per the task's "say so
  if a test's purpose changes" instruction, though the *purpose* here did
  not change, only the fixture's directory layout matching the new anchor
  depth.

## On the escape test's actual teeth (honesty note)

The task named `a_relative_escape_through_the_workspace_segment_is_refused`
(entry `"$WORKSPACE_ROOT/../sibling-key"`-shaped) as one of the two most
important new assertions, framed as "escapes the NEW narrower anchor even
though it would have stayed inside the OLD shared-parent anchor." Direct
computation (confirmed by the already-existing, already-passing pre-fix
test `an_anchor_sibling_sharing_a_byte_prefix_is_not_contained`, which
pins `"$WORKSPACE_ROOT/../workspaces-evil"` as `Invalid`/`escapes` against
the UNFIXED code) shows this framing is imprecise: `contain_within_anchor`
checks position relative to whatever `root` currently is, and stepping one
level up from `root` escapes `root` regardless of `root`'s absolute depth
— so a pure `..`-shaped escape off `$WORKSPACE_ROOT` was ALREADY refused
before this fix, for a structurally different but outcome-identical
reason (it escaped the bare, wide anchor by exactly the same relative
shape it now escapes the narrow, per-call one by). The genuine,
provable R-1 vulnerability is the K=0 case — the BARE `$WORKSPACE_ROOT`
token, no traversal at all, resolving to the entire shared parent — which
is exactly what `two_different_workspaces_resolve_to_disjoint_writable_roots`
regression-tests, and which DOES flip Err/wrong-value under the
un-fixed code (confirmed in the teeth section below). I kept
`a_relative_escape_through_the_workspace_segment_is_refused` as a real,
true, useful pin (it proves COR-2's containment machinery is now invoked
against the CORRECT, narrower anchor, not the bare one — verifiable by its
error text naming the per-call entry), but I am not overclaiming an
old-vs-new outcome flip for this specific test that the code does not
actually exhibit. This precision is recorded here per the project's
bounded-claims discipline.

## Note on p5_hard_gate.rs scope beyond the task's two named call sites

The task names two call sites needing a pre-created directory: the
`live_govern` helper (~760-792) and the shipped-artifacts section
(~1178-1220). Investigation shows the shipped-artifacts section is NOT a
separate test — it is the tail of
`containment_wraps_from_process_start_and_confines_the_session` (starts
line 1074), reusing the SAME `workspace`/`home` scratch dirs as the
`live_govern("wraps-from-start", ..., &home, &workspace)` call earlier in
that same test body. That test also does REAL in-sandbox writes asserted
against `workspace.join("inside.txt")` directly (today, workspace IS the
writable root under the old bug). After the fix, the writable root becomes
`workspace.join("p5-gate-workspace")` (a strict subdirectory) — the parent
`workspace` itself is now only read-only-visible (baseline `--ro-bind / /`
in the bwrap adapter, `crates/adapters/gauntlet-adapter-sandbox-bwrap`).
So the in-sandbox write assertions must target the new writable root, not
bare `workspace`, or the "inside write succeeds" assertion goes red for a
reason unrelated to a real defect (it would be testing the wrong path).
This is required to keep the test's own claim true, not a scope expansion:
still only editing `p5_hard_gate.rs`, no new test files, no weakened
assertions — the write target is corrected to the location the fixed code
actually grants.

## Residual R-1b (verbatim, per operator's explicit scope boundary)

`crates/gauntlet-app/src/govern/gate.rs`'s `GovernDeps` holds ONE
`workspace: WorkspaceRef` field, set once at construction, and
`AuthorityGate::admit` calls `.policy_for(facts.tier, &deps.workspace,
deadline)` using that SAME stored ref for every unit dispatched through
that `GovernDeps` instance for its whole lifetime — never a fresh
per-unit workspace. Fixing THAT (composing a real `WorkspaceProvider` and
threading a fresh per-unit `Workspace`/`WorkspaceRef` into the gate) is
genuinely Task 15/16 scope: it requires a production `WorkspaceProvider`
adapter that does not exist yet (only test stubs do, in
`crates/gauntlet-ports/src/contract/stubs.rs`), and touches controller
composition that has no production caller today. NOT attempted here. Did
not edit `gate.rs`, `controller.rs`, or anything under
`crates/gauntlet-app/src/control/`.

## Closing summary

**What changed and why.** `PolicyFs::policy_for` ignored its `workspace:
&WorkspaceRef` parameter and resolved every policy entry's
`$WORKSPACE_ROOT` token against the bare, construction-time
`anchors.workspace_root` — the shared parent directory every run's and
every unit's workspace lives under, not any one caller's own workspace.
Under the shipped `.gauntlet/policies/*.json` (`"writable_roots":
["$WORKSPACE_ROOT"]`), every dispatched builder was granted read-write
over every other unit's and run's workspace. The fix validates the
workspace ref as a safe path segment (`validated_workspace_segment`,
closing the same anchor-escape class COR-2 closed for policy-entry paths,
applied one level up to the workspace ref itself — `WorkspaceRef::new`'s
own validation does not reject `..`, `.`, or embedded `/`), then scopes
`$WORKSPACE_ROOT` per call to `anchors.workspace_root.join(<validated
segment>)` before passing that narrower, owned `AnchorRoots` through the
EXISTING, unmodified §17.5.2 containment machinery (`resolve_anchored_path`
/ `contain_within_anchor`, COR-2/COR-5). Doc comments (module header,
`AnchorRoots.workspace_root`, `resolve_anchored_path`) were corrected to
describe this scoping instead of the old (wrong) "bare anchor" resolution.
Three new regression tests pin the fix; five pre-existing tests were
updated to the new (correctly scoped) expected values, with each change's
purpose recorded in the "test survey" section above. `p5_hard_gate.rs`'s
two `PolicyFs` fixture call sites now pre-create the exact per-call
workspace subdirectory the fixed code resolves against (mirroring what a
real `WorkspaceProvider::create()` would already have done in production
— `PolicyFs` itself still never creates a directory), and the one test
that does REAL in-sandbox writes was retargeted to the writable root the
fix actually grants.

**Residual R-1b (verbatim, unchanged from the operator's scope boundary):**

> `crates/gauntlet-app/src/govern/gate.rs`'s `GovernDeps` holds ONE
> `workspace: WorkspaceRef` field, set once at construction, and
> `AuthorityGate::admit` calls `.policy_for(facts.tier, &deps.workspace,
> deadline)` using that SAME stored ref for every unit dispatched through
> that `GovernDeps` instance for its whole lifetime — never a fresh
> per-unit workspace. Fixing THAT (composing a real `WorkspaceProvider`
> and threading a fresh per-unit `Workspace`/`WorkspaceRef` into the gate)
> is genuinely Task 15/16 scope: it requires a production
> `WorkspaceProvider` adapter that does not exist yet (only test stubs do,
> in `crates/gauntlet-ports/src/contract/stubs.rs`), and touches
> controller composition that has no production caller today. NOT
> attempted here. Did not edit `gate.rs`, `controller.rs`, or anything
> under `crates/gauntlet-app/src/control/`.

**Exact gate numbers** (full detail in "Final gate results" above):
`just check` PASS; `gauntlet-adapter-policy-fs` nextest 20/20 PASS;
`gauntlet-app govern:: doctor::` 44/44 PASS (302 skipped, expected);
`p5_hard_gate` + `g7_tier_downgrade` 7/7 PASS (including the real
sandboxed-execution leg); `just lint` (clippy zero warnings + fmt clean)
PASS; `just boundaries` 215/215 PASS (after fixing one denylist violation
my own new comment introduced — see "Mid-course correction" above);
`just doctor` exit OK; `just ruler` PASS.

**Teeth evidence** (full detail in "Step 6 teeth evidence" above):
baseline sha256 `a6a682285350d673a21eb2220d356dc8ad3cb25fc8337406d287fa0f9814be1b`.
Leg 1 (revert scoping to `&self.anchors`):
`two_different_workspaces_resolve_to_disjoint_writable_roots` went RED
(`left: ["/run/workspaces"] right: ["/run/workspaces"]` — both callers
handed the bare shared parent, R-1's exact defect);
`a_relative_escape_through_the_workspace_segment_is_refused` stayed GREEN
(honestly reported, not overclaimed — see the dedicated honesty note
above for why this specific traversal-shaped test doesn't flip under this
mutation). Byte-identical restore confirmed by sha256 match; both tests
GREEN again; full package 20/20. Leg 2 (disable the validator,
`let sanitized = true;`): `an_unsanitized_workspace_ref_is_refused` went
RED (`workspace ref ".."` resolved `Ok` with `writable_roots: ["/run"]`
— the scoped anchor walked back up past the shared parent). Byte-identical
restore confirmed by sha256 match; full package 20/20 GREEN.
