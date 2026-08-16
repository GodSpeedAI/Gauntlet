# Current status — HANDOFF DOCUMENT

**Last updated:** 2026-08-16 — Task 2 in its **fifth** round (round-5 correction in flight).
**Read this file first.** Then `AGENTS.md`, `.agents/specs/Core-spec.md`,
`.agents/plans/gauntlet-impl.md` — in that order.

---

## TL;DR for the next agent

Tasks **0 and 1 are complete, gate-passed, teeth-checked, and committed.** The ruler is
frozen and must never be edited.

**Task 2 is committed (`b69c095`) but REOPENED. Task 3 has not started and must not start
until Task 2 verifies clean of criticals.**

The history matters, because it is the argument for the whole method:

| Round | What happened |
|---|---|
| 1 | First cut. All five gates green. |
| 2 | Three blind critics → **34 residuals, 4 critical.** `.agents/reports/task2/recritique-round2.md` |
| — | Correction round. Suite 31 → 81 tests. Gates green. Seven teeth cases demonstrated. Committed as `b69c095`, explicitly labelled *provisional, not independently reviewed*. |
| 3 | Two blind critics → **19 residuals, 4 critical.** `.agents/reports/task2/verification-round3.md` |
| — | Correction round. Suite 81 → 113 tests. Gates green. |
| 4 | Four blind critics (`critic4/critic5`, × evasion + vacuity lenses) → **4 critical, 18 major, 13 minor.** Raw reports in `.agents/reports/task2/critic{4,5}-{evasion,vacuity}.md` |

**A green gate has now been wrong three times.** Each time the gate passed, the teeth bit, and a
blind critic still found concrete ways to compile infrastructure into the domain layer. That
is the entire reason the blind-critic step is non-negotiable and why `INCONCLUSIVE` is never
`PASS`.

### Round-4 criticals and how round 5 triages them

| # | Finding | Trigger class | Disposition |
|---|---|---|---|
| C-A | External-dependency channel entirely unpoliced. `declared_deps` classifies a dep only if it resolves to a workspace member; a plain registry dep (`serde = "1"`) matches no branch and is invisible. | **accidental** — an ordinary mistake, not an evasion | **fix**: per-layer external-dep policy, deny-by-default |
| C-B | `check_cargo_config` probes only `[source.*]` and `[build].rustflags`; cargo also honours `[target.*] rustflags`, so `--extern` puts an arbitrary rlib in the extern prelude. | adversarial | **fix** — cheap |
| C-C | External allowlist admits by *name*, never source kind; the suite's own `toml` parser can be swapped for a git fork. | adversarial | **fix** — cheap |
| C-D | `attribute_body_end` counts brackets with no string-literal state, so a `]` inside a predicate string truncates the body and hides a `#[cfg_attr(…, path = …)]`. | adversarial | **fix** — genuine parser bug |

C-A is the one that matters and the reason round 5 exists: it is reachable by an ordinary
mistake, and AK-001 says the domain layer has *zero* compile-time dependencies on
infrastructure. A registry crate is infrastructure.

### Round 5 — RESULT (2026-08-16). Work is ON DISK and UNCOMMITTED.

Builder closed all seven briefed items (C-A…C-D, M-1…M-3) plus the threat model. Suite is
**113 → 159 tests, 159 passed, 0 skipped**; `check`/`lint`/`deny`/`boundaries`/`ruler` all
exit 0; `target/` 360M. Teeth verified by mutation (four mechanisms broken, 2–5 failures each,
all reverted). Report: `.agents/reports/task2/builder-round5.md`.

Three things the builder found that no critic did, all now closed: cargo reads `.cargo/config.toml`
from the invocation directory **upward**, so a root-only probe missed
`crates/gauntlet-domain/.cargo/config.toml`; `rust-toolchain.toml`'s `[toolchain] path` replaces
the compiler through the same channel; and `dep = { workspace = true, registry = "mirror" }`
narrows an inherited entry to another index. It also reported honestly that C-A was **already
closed on arrival** (round 4 had done it — my brief was stale) rather than claiming it as new,
and that C-D was half-done with **no test anywhere**.

Two blind critics (`critic6-evasion`, `critic7-vacuity`) → **5 critical, 6 major, 9 minor.**

**The convergence signal.** critic6 states it worked every manifest and source-inclusion
channel — `[patch]`/`[replace]`, target-scoped tables, workspace inheritance with renames,
alternate registries, git sources, path escapes, undeclared packages, in-repo `.cargo/config*`
at every depth, `rust-toolchain.toml`, all five target-path overrides, `build.rs`, `#[path]`,
`cfg_attr`, every `include!` spelling, escaped and continuation-split arguments, symlinks —
and that **they all held**. The C-A…C-D class is closed.

**All five round-5 criticals are downstream of one subsystem: the lexer in `text.rs`.**

| # | Finding | Inside stated boundary? |
|---|---|---|
| c7-C1 | Declared-defaults exemption is **file**-scoped, not **setting**-scoped. `check_defaults_exemption` only asserts six `const <setting>` substrings are *present*; nothing bounds what else the file declares, and all 13 literal classes are off for it. 8 of §8.1's 9 classes become hardcodable in `gauntlet-app`. | yes — **and it goes live at Task 5**, which creates exactly that file |
| c7-C2 | A directive head split before its `=`/`(` is never joined, so `include!`⏎`("../../shared/wiring.rs")` matches nothing. Backstopped only by the relative-path class — which c7-C1 or MJ2 switches off. | yes |
| c6-C1 | `string_literals` mis-pairs quotes: a `'"'` char literal earlier on the line deletes every following literal from eight classes **and** from AK-014's adapter-segment rule. Builder left this deliberately ("not in the same round as four criticals"). | yes |
| c6-C2 | Timeout class only sees values introduced by `=`, `(` or `,`; a struct-field initialiser hides every duration. | yes |
| c6-C3 | `find_tier_policy` disables itself for any unit containing `#[` — including in a comment. | yes |

All five are inside the enforced surface, so by the rule below **all five reopen Task 2.**
None is an environment or third-party-contents escape.

**Round 6's brief writes itself, and it is not "patch five classes."** Every one of these is a
consequence of `text.rs` doing ad-hoc line-based joining and quote-counting instead of lexing
Rust. Round 6 should build **one** string/char/raw-string/comment-aware lexer pass that every
class consumes, and delete the per-class ad-hoc scanning — plus make the defaults exemption
setting-scoped rather than file-scoped (c7-C1 is independent of the lexer and is the one that
blocks Task 5). Majors to fold in: I8 violations where an unreadable `.cargo/config*` or
unparseable `rust-toolchain*` is silently skipped (c6-M1/c7-MJ1 — this contradicts the crate's
own freshly written threat model); `RELATIVE_PATH_EXEMPTIONS`/`ENDPOINT_EXEMPTIONS` are the only
inventories with no content pin, so one added row un-polices a literal forever (c7-MJ2);
`check_scope_integrity` is the only repo-level check with no coverage floor (c7-MJ3).

### Terminating this loop honestly

Four rounds have each produced four criticals, and the triggers are drifting steadily from
"ordinary mistake" toward "attacker who already controls the developer's environment." That
drift is the signal that the missing artifact is not another fix — it is a **stated threat
model**. Round 5 therefore requires, as a deliverable, a module-level threat-model section in
`crates/gauntlet-arch-tests/src/lib.rs` naming what the suite enforces (the repository's own
declared build inputs, evasion-resistant within that surface, I8-honest about unscannable
input) and what it does **not and cannot** enforce, each with a reason: the developer's
environment outside the repo (`$CARGO_HOME`, `RUSTFLAGS`, a modified toolchain), the
*contents* of an admitted third-party crate, proc-macro/build-script codegen beyond the
path-resolution checks, and runtime string assembly.

This is what lets a reviewer tell *"we did not think of it"* from *"we drew the line here,
deliberately, and here is where it is."* After round 5, a surviving critical whose trigger
lies **outside** that stated boundary is a recorded deferral, not a reopen. One whose trigger
lies **inside** it still reopens the task. Without that rule this loop does not terminate,
and an unterminated loop is how Tasks 3–20 never get built.

All four round-3 criticals are one class — **what gets scanned ≠ what gets compiled**:
`[lib]`/`[[bin]]`/`[[test]] path` overrides pointing outside the crate; `#[cfg_attr(…, path =
…)]` evading a literal `"#[path"` matcher; `include!(concat!(env!("OUT_DIR"), …))` resolving
to a harmless in-crate path while `build.rs` codegen compiles unscanned; and workspace-
inherited renames (`store = { package = "rusqlite" }` + `store.workspace = true`) invisible
because the root `Cargo.toml` is in no scan scope at all.

Sharpest of them: the suite's **own** `Cargo.toml` uses `[[test]] path = "../../tests/…"`, so
it demonstrably knows that channel exists and leaves it unpoliced.

**Rule this establishes for every later task:** a task is not done when its gate is green. It
is done when its gate is green *and* a blind critic that never saw the builder's reasoning
fails to break it.

**Before you touch anything:** run `just ruler`. It must print PASS. If it does not,
that is a higher-priority problem than any task — stop, diagnose, and report it in
`.agents/reports/` before doing anything else.

**Build-lock discipline (operator instruction):** only one agent may compile at a time,
and `target/` must be watched (`du -sh target`; `cargo clean` if it grows past ~4 GiB).
Blind critics are read-only and non-compiling; builders hold the lock exclusively.

---

## Completed work

| Task | State | Commit |
|---|---|---|
| 0 — Recon, toolchain baseline, delegation spike | **done** | `76e491a` |
| 1 — Freeze the ruler | **done** | `bf09a30` |
| — toolchain install bookkeeping | done | `ac0f033`, `66eebc0` |
| 2 — Workspace skeleton + justfile + gates + boundary test | **REOPENED** — committed but 4 critical findings open | `b69c095` |
| 3 — Domain types | not started — gated on Task 2 verifying clean | — |
| 4–27 | not started | — |

### Task 2 — where it actually stands

Built: 16-crate workspace (`gauntlet-{domain,ports,app,cli}`, 11 adapters,
`gauntlet-arch-tests`), extended `justfile` (all ten recipes; `ruler` untouched),
`.env.example` (92/92 Annex E vars, all empty), `deny.toml` (real policy).

Gate `just check && just lint && just deny && just boundaries && just ruler` exits 0.

**Teeth check demonstrated by the orchestrator** (independent of the builder's own run):
`sqlite` and `reqwest` dependency keys and a model-id literal each added to
`crates/gauntlet-domain` made `just boundaries` exit 100; reverts confirmed byte-identical
by `cmp`; gate green again after. The `hardcoded-model-id` literal class was separately
confirmed to fire on its own (`gpt-4o`), so it is not dead code masked by the denylist.

**Why it is still not done:** a passing gate with holes in it is exactly the failure Task 2
exists to prevent ("a weakened boundary test is worse than none because it looks like
coverage"). The four critical findings let infrastructure into the domain layer *today*
with every gate green — e.g. a path dependency on a crate absent from `[workspace].members`,
a symlinked source file, and `libsqlite3-sys` as a domain dependency key (the manifest
matcher was weaker than the source matcher).

### Design decisions taken during Task 2 — preserve these

- **Composition-root carve-out.** The adapter-identifier rule (AK-002, §8.1) applies to
  `gauntlet-domain`, `-ports`, `-app` — **not** to `gauntlet-cli`, which spec §4.1 makes the
  composition root and which must be free to name and declare adapter crates. `gauntlet-cli`
  remains subject to the hardcoded-literal scan. This was decided deliberately *now* because
  otherwise the denylist collides with its own composition root at Task 20 and would have to
  be weakened under deadline pressure.
- **`GAUNTLET_RESULT_PATH` is deliberately absent from `.env.example`.** It appears in the
  spec (§10.3, Annex C) but is runtime-injected per child invocation, not operator
  configuration, and is correctly not in Annex E. Listing it would imply operators may set it,
  and a stale value could redirect a result contract. It is encoded as a **named, justified
  exclusion** in the `env_example_complete` instrument — never a silent omission.
- **Evidence-locator obligation.** The frozen AK units name locators under `tests/contract/`
  (`architecture_boundaries`, `::denylist`, `no_hardcoded_literals`, `env_example_complete`).
  Task 2's instruments must be addressable at those names or Task 27 cannot resolve evidence
  for AK-001/002/016/019. Being worked in the current round; if cargo refuses the
  out-of-package test path, the mismatch is recorded rather than papered over.

### The `versioning` rename — APPLIED

The operator instructed: *"no implementation details should be hardcoded… such variables and
related names should be `versioning`… to not lock in git."* Applied atomically across the spec
and `.env.example` (they must move together — `env_example_complete` parses Annex E out of the
spec and diffs it, so a one-sided rename fails the gate). Changes made:

| File | Change |
|---|---|
| `.env.example:58` + `Core-spec.md:2091` | `GAUNTLET_TOOL_GIT` → `GAUNTLET_TOOL_VERSIONING` |
| `.env.example:98` + `Core-spec.md:2093` | `GAUNTLET_GIT_STRATEGY` → `GAUNTLET_VERSIONING_STRATEGY` |
| `Core-spec.md:466` | `Observer` required capability `git` → `versioning` |
| `Core-spec.md:445` | instrument-class list `git` → `versioning` |
| `Core-spec.md:534` | `facets: [git, …]` → `facets: [versioning, …]` |

This completes the rename the operator began at `Core-spec.md:573` and `:886`. **`.env.example`
and Annex E must change together** — the `env_example_complete` instrument parses Annex E out
of the spec and diffs it, so a one-sided rename fails the gate.

**Ruler safety: confirmed.** `.wayfinder/answer-key.md` and `.wayfinder/reference.yaml`
contain **zero** occurrences of `git` or `versioning`. No rename can move the ruler; `just
ruler` is unaffected.

**Deliberately NOT renamed: `crates/adapters/gauntlet-adapter-observe-git`** and the `git`
entries in the spec's *adapter* rows (`:144`, `:1369`) and concrete git usages (`:453`,
`:1072`, `:1325`, `:2131` — worktrees, `.git` directory denial). The adapter layer is the one
place product names are correct and required (§24.3 permits product identifiers **only**
under `crates/adapters/**`). A git-named adapter behind a `versioning` port is what *prevents*
lock-in; renaming it to `observe-versioning` would make a git-only implementation look generic
and leave nowhere to add `observe-jj` or `observe-hg`. Flagged for the operator; reversible in
one command if they disagree.

Consequence for later tasks: Task 4's `Observer` port must use `Versioning` as the facet-kind
variant, never `Git`.

### Operator edits to the spec, absorbed

The operator renamed the `git` tool capability and observation facet to `versioning`
(`Core-spec.md:573`, `:886`). Annex A is untouched, `just ruler` still passes, and the
frozen answer key never mentions either word — **no ruler conflict.** Task 4 (ports) and
Task 9 (observers) must use `versioning`, not `git`, as the capability/facet name.

### Task 0 outcomes

- `.agents/reports/environment.json` + `.md` — full environment fingerprint.
- `.agents/reports/delegation-spike.md` — **the delegation question is settled
  empirically.** This harness's `Agent` tool is admission-based: the call returns an
  admission handle synchronously (agentId + output-file ref + explicit "you know nothing
  about its results yet"), and the real result arrives later as a separate
  `<task-notification>`. This matches spec §10.5/Annex C's analysis of Prime-Agent's
  `rlm(...)` exactly. **Consequence: no `AgentRunner` port adaptation is needed. Build
  Task 4 on the admission-based design as specified** (`invoke -> handle`, `poll`,
  `collect`, result-contract file). Do not re-spike this.
- Directory skeleton per spec §25 created with `.gitkeep`s. The `<source tree>`
  (`crates/...`) was deliberately left to Task 2, which owns that layout.

### Task 1 outcomes — THE FROZEN RULER

- `.wayfinder/answer-key.md` — spec Annex A, **verbatim**, extracted by `sed` line-range
  (never retyped), byte-identical diff confirmed. 70 Units, AK-001…AK-070.
- `.wayfinder/reference.yaml` — manifest wrapper (spec §5.3.1). Units are *referenced*,
  not duplicated, so there is only one copy of the 70 claims.
- `.gauntlet/reference.lock` — the freeze record:
  - answer key: `sha256:ccc7c99be3d3b60488930eed9975dc6d67378a84bf3b13910b48511e9c228859`
  - manifest: `sha256:95331d3e345fa799e9840dbdb02b60b7c41c73be84c96fb7ecc1068775ba5e94`
- `scripts/reference-digest.sh`, `scripts/verify-ruler.sh`, `justfile` (`just ruler`).

**Design decision you must preserve (Task 7 depends on it):** the digest script
normalizes CRLF→LF and collapses end-of-file newline drift, and does **nothing else** —
it deliberately does *not* strip per-line trailing whitespace. The plan's Task 1 step 3
loosely said "strip trailing whitespace", but that same task's teeth check requires that
appending a single space to any line breaks the digest. Both cannot hold. Resolved in
favour of ruler sensitivity (spec §9.4 makes normalization implementation-defined as long
as it is deterministic and documented). **Task 7's Rust `normalize` must reproduce these
exact rules** — the two implementations agreeing is the Task 7 gate.

---

## Environment — do not re-derive this

| Thing | State |
|---|---|
| Host | WSL2, kernel 6.18.35.2, x86_64, **6 cores, 7.8 GiB RAM (tight — ~1.6 GiB free)** |
| Rust | rustc/cargo 1.92.0, rustup 1.29.0, rust-analyzer 1.92.0 |
| `just` | 1.58.0 |
| `cargo-nextest` | 0.9.143 ✅ installed |
| `cargo-deny` | 0.20.2 ✅ installed |
| `cargo-mutants` | 27.1.0 ✅ installed |
| Other | git 2.53.0, sqlite3 3.46.1, python3 3.14.4, node 24, bun 1.3.14, uv 0.12.1, gh 2.46.0 |
| **Sandbox** | **`bwrap` 0.11.1 only** (smoke-tested OK). **No docker, podman, qemu, firecracker.** |

**Memory is the real constraint.** Rust compiles under 7.8 GiB with ~1.6 GiB free. Keep
`max_parallel` low (spec default 2 is right here); do not fan out many concurrent
Rust-compiling builders or you will OOM.

**Containment consequence (already recorded, don't re-litigate):** `contained` and
`local` tiers get genuine bwrap containment. `cross_cutting` will also run under bwrap as
the strongest *available* real backend — weaker than the container/microVM that spec
Annex F calls typical. This is a real, honestly-recorded weakness destined for
`docs/CONFORMANCE.md` at Task 27. It is **not** a Task 14 redesign trigger (that fires
only if *no* real backend exists). `irreversible` gets **no** policy file, by design.

---

## Rules that outrank whatever task you are on

1. **Never edit `.wayfinder/answer-key.md`.** Not for a failing gate, not for anything.
   A Unit that seems wrong becomes a dated, evidenced entry in
   `.agents/reports/reference-candidates.md` (file not yet created — create it when
   first needed). You keep building against the frozen version.
2. **The spec outranks the plan.** On conflict, fix the plan; never the spec. (Task 1
   already did this once — see the digest-normalization note above.)
3. **`just ruler` before and after every task.** A failure there beats the task in flight.
4. **Delegate; do not implement directly.** AGENTS.md §"Delegation and role boundaries"
   and spec §10.7. Every builder slice gets an independent **blind** critic that receives
   only the artifact + relevant answer-key units — never the builder's reasoning,
   transcript, or attempt count.
5. **Every task needs its gate at exit 0 AND a demonstrated teeth check** (break the
   mechanism, watch the gate fail, revert, record it in the commit message). A task
   without a demonstrated teeth check is not done. See `bf09a30` for the format.
6. **Parallel slices get separate git worktrees**, never a shared checkout.
7. **`INCONCLUSIVE` is never `PASS`**, at any layer, ever.
8. **One task, one commit**, with gate + teeth output in the message.
9. Irreversible-tier work, human sign-off, or a genuine blocker → record in
   `.agents/reports/escalations.md` (not yet created), skip to the next unblocked task,
   continue. **Never downgrade a tier or authority to keep moving.**

---

## Answer-key unit status (spec §26 categories)

Interim, as of the Task 2 correction round. Task 1 froze the *standard* these Units are
graded against; it did not satisfy any of them. Task 2 is the first task to touch any Unit.

- **implemented and proven: none.** No Unit has settled. Nothing here has a
  `VerificationRecord`, because the verification machinery does not exist until Task 11.
- **implemented but provisional (3):** **AK-001** (dependency direction), **AK-002**
  (adapter identifiers absent from domain/application), **AK-016** (no hardcoded literals).
  A mechanism exists for each, `just boundaries` enforces it, and the teeth were
  demonstrated. They are **provisional, not proven**, for two independent reasons: the
  blind critics found concrete evasions still open (see
  `.agents/reports/task2/recritique-round2.md`), and the evidence locator each Unit
  declares under `tests/contract/` does not yet resolve to a runnable target.
- **configured but untested (1):** **AK-019**. `.env.example` is correct — independently
  verified against Annex E at 92/92 variables, all values empty, no credentials — but until
  the `env_example_complete` instrument lands, nothing mechanically holds it that way.
  Correct-by-inspection is not the same as enforced, and saying so costs nothing.
- **proposed (66):** every other Unit, including **AK-014** — nothing selects an adapter
  yet, so "adapter selection resolves from configuration only" is currently *vacuously*
  true and has no instrument. Vacuous truth is not evidence.
- **blocked: none.**
- **unknown: none.**

No Unit above should be reported as satisfied. Three have mechanisms with known holes; one
has an artifact with no mechanism; the rest have neither.

---

## Open unknowns (recorded in `.wayfinder/reference.yaml`, not silently resolved)

- **UNK-001** — which containment backend `cross_cutting` runs under in real use, beyond
  this host's bwrap-only environment.
- **UNK-002** — should this repo carry a LICENSE, and which? Not evidenced anywhere; not
  invented. No LICENSE file added.
- **UNK-003** — which concrete `AgentRunner` should be `runners.default` in `GAUNTLET.md`.
  Deferred to Task 5 / Tasks 15–16.

## Escalations

**ESC-001 CLOSED** (2026-08-15) — delegation capacity was exhausted mid-round and reported as
`blocked{no_delegation_mechanism}`; Task 3 was deliberately NOT implemented inline, because the
plan's guardrail forbids the orchestrator implementing when delegation is unavailable and doing
so would also have destroyed the independent-blind-critic property. The limit reset, delegation
resumed, and the deferred blind verification ran — **and found four criticals.** The escalation's
stated risk ("do not upgrade AK-001/002/016 beyond *implemented but provisional*") was correct,
and holding that line is what caught them. Full context in `.agents/reports/escalations.md`.

No escalations are currently open.

---

## Next action, concretely — READ THIS FIRST (supersedes the older list below)

Round-5 work is **on disk and uncommitted** (`crates/gauntlet-arch-tests/src/*.rs`, +6290/−336).
Gates were green in the builder's own run; the orchestrator has **not** independently re-run
them, so that is INCONCLUSIVE, not verified.

1. Run the gate yourself: `just check && just lint && just deny && just boundaries && just ruler`.
   Then run the independent teeth script at
   `…/scratchpad/teeth3.sh` (staged, executable; covers the four round-4 criticals plus two
   regressions). A non-zero exit that comes from *cargo* rather than from our own violation
   message is INCONCLUSIVE and must be recorded as such — that trap already caught us once.
2. Commit round 5 (one task, one commit; gate + teeth in the message). Stage explicitly;
   **never** stage `.jolli/`. Label it in the message as *round 5, five criticals still open*.
3. Round 6, per the brief above: one real Rust lexer in `text.rs`, plus the setting-scoped
   defaults exemption. One builder holding the build lock, then two blind critics.
4. Then **Task 3**. Its workflow script is already written and staged at
   `…/scratchpad/task3-wf.js` — fire it with
   `Workflow({scriptPath: …})`. It carries the full spec pointers (Core-spec.md lines 185–404),
   the zero-dependency constraint, AK-003…AK-007, and two blind critic lenses
   (constructibility of invalid states; spec fidelity + test vacuity).

**Do not let Task 2 consume the remaining budget.** If round 6's criticals are again confined to
one identified subsystem and are again all *adversarial* rather than *accidental* triggers, the
correct move is to record them as accepted residuals under the stated threat model, commit, and
move to Task 3 — Tasks 3–20 do not exist yet, and a perfect boundary suite guarding an empty
repository proves nothing. The accidental-trigger class (c7-C1, which Task 5 walks straight
into) is the one that must be closed before moving on.

## Older next-action list (Task 2 rounds 1–4 era; kept for context)

**Finish Task 2, then commit it.** It is built and green but carries 34 recorded residuals;
a correction round is in flight. To close it out:

1. Confirm the correction round landed: `just check && just lint && just deny && just
   boundaries && just ruler` all exit 0.
2. Re-run a blind critic on the corrected artifact. Any surviving **critical** finding means
   Task 2 is not done — correct again. Majors and minors may be carried forward as recorded
   deferrals, but only *recorded*, never silently.
3. Re-demonstrate the teeth (the plan's own: `sqlite`/`reqwest`/a model-id literal in
   `crates/gauntlet-domain` must fail `just boundaries`), revert byte-identical, and put the
   gate + teeth output in the commit message.
4. Commit — one task, one commit. Stage explicitly; **never** stage `.jolli/` (plugin
   working-tree noise, deliberately excluded from every commit so far).

Then **Task 3 — Domain types** (spec §5.2–§5.3). Its shape: every value object and enum as a
type with invalid states unrepresentable; `Outcome` as a three-state enum with **no**
`From<Outcome> for bool` and no `is_ok()` shortcut; aggregates `Reference`, `Unit`, `Run`,
`Iteration`, `RoleInvocation`, `Budget` plus the record types; constructor-level invariants
(`irreversible ⟹ human`; `cross_cutting|irreversible ⟹ authority ≠ critic`; `settled`
requires evidence + verification refs) returning typed errors, never panics; property tests
for round-trips, unknown-enum-is-error, and the tier/authority coupling.

Gate: `cargo nextest run -p gauntlet-domain && just boundaries`
Teeth: constructing `Unit{blast_radius: irreversible, authority: critic}` must fail to
compile or return a typed error.

Decompose and delegate both; do not implement inline. One compiling agent at a time.
