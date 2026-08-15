# Current status — HANDOFF DOCUMENT

**Last updated:** 2026-08-15 — Task 2 in its second correction round (uncommitted).
**Read this file first.** Then `AGENTS.md`, `.agents/specs/Core-spec.md`,
`.agents/plans/gauntlet-impl.md` — in that order.

---

## TL;DR for the next agent

Tasks **0 and 1 are complete, gate-passed, teeth-checked, and committed.** The ruler is
frozen and must never be edited.

**Task 2 is complete, gate-passed, teeth-checked, and committed.** **Start at Task 3.**

Three independent blind critics found **34 residuals (4 critical, 19 major, 11 minor)** in the
first cut of the boundary gate — concrete, reproducible ways to violate the layering with
`just boundaries` still green. Full findings: `.agents/reports/task2/recritique-round2.md`.
A correction round closed them; the suite went from 31 to 81 tests.

**One thing the next agent must finish:** the correction round's own blind re-critique never
ran — both critics died on the session limit (workflow `wf_8d6d78e3-347`, all three agents
errored). So the corrected artifact has a demonstrated teeth check but **no independent
review**. Run a blind critic over `crates/gauntlet-arch-tests/` before relying on it, and
treat any surviving critical finding as a Task 2 reopen rather than a Task 3 problem.

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
| 2 — Workspace skeleton + justfile + gates + boundary test | **gates green, correction round 2 in flight, UNCOMMITTED** | — |
| 3–27 | not started | — |

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

**None open.** `.agents/reports/escalations.md` does not exist yet — create it when the
first escalation occurs.

---

## Next action, concretely

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
