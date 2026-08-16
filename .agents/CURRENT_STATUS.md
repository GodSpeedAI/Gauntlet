# Current status — HANDOFF DOCUMENT

**Last updated:** 2026-08-16 — **Task 2 CLOSED at round 6.1. Task 3 delegated and in flight.**
**Read this file first.** Then `AGENTS.md`, `.agents/specs/Core-spec.md`,
`.agents/plans/gauntlet-impl.md` — in that order.

---

## TL;DR for the next agent

Tasks **0, 1 complete** (ruler frozen — never edit `.wayfinder/`). **Task 2 is CLOSED**:
seven critique rounds, final state 183/183 arch tests green, all five global gates exit 0,
teeth demonstrated independently by the orchestrator on every critical class. Surviving
residuals are **recorded deferrals** (ledger below), none critical, none accidental-trigger
critical. **Task 3 (domain types) was delegated this session** — see "Next action".

The history is the argument for the whole method — a green gate was wrong three times
before the blind-critic loop caught it. Keep the loop: **a task is done when its gate is
green AND a blind critic that never saw the builder's reasoning fails to break it**, with
the round-5 threat-model rule deciding what reopens vs. defers.

| Round | What happened |
|---|---|
| 1 | First cut. All five gates green. |
| 2 | Three blind critics → 34 residuals, 4 critical. Correction; committed `b69c095` (provisional). |
| 3 | Two blind critics → 19 residuals, 4 critical (what gets scanned ≠ what gets compiled). Correction; 113 tests. |
| 4 | Four blind critics → 4 critical (C-A…C-D), 18 major, 13 minor. |
| 5 | Correction closed C-A…C-D + threat-model deliverable; 159 tests. Two blind critics → 5 critical (all in the `text.rs` lexer). Work landed as commit `f85c925` — see deviation note below. |
| 6 | One real lexing pass (`text::regions`), setting-scoped defaults exemption, I8-honest declared inputs, pinned inventories. Two blind critics (critic8-evasion, critic9-vacuity) → **2 critical (E1, E2), 9 major, 13 minor**. |
| 6.1 | Closed E1 (comment tokens inside directives), E2 (`[env] RUSTFLAGS` cargo-config channel), F2/E7 (declaration-granular defaults exemption). **183 tests. 0 critical open.** CLOSED. |

### Round-6/6.1 closure record

- Builder reports: `.agents/reports/task2/builder-round6.md`, `builder-round6.1.md`.
  Critic reports: `critic8-evasion.md`, `critic9-vacuity.md` (raw, unfiltered).
- **Orchestrator verification (independent of builders' own runs):** full gate
  `just check && just lint && just deny && just boundaries && just ruler` exit 0
  (183/183); repo-level teeth, each failing `just boundaries` with the suite's own
  violation message then reverting byte-identical: round-5 set (denied dep key; plain
  registry dep into domain; crate-depth `.cargo/config.toml` `--extern` rustflags;
  `cfg_attr` path escape; model-id literal) and round-6.1 set (E1 comment-in-directive
  payload; E2 `[env] RUSTFLAGS` payload — flagged as "the `[env]` spelling of the
  rustflags key this check already denies (AK-001, spec §24.3)"; F2 `localhost:8080`
  endpoint on a setting line inside a fully-warranted `defaults.rs`).
- Known-gaps the 6.1 builder declared honestly: cross-line directive heads with trailing
  comments (`include! /*c*/`⏎`("…")`) still defeat head-join (declared in lib.rs
  residual list); `[env]` names reaching the compiler only transitively (`PATH`,
  `LD_LIBRARY_PATH`) not denied — recorded policy call, not taken.

### Deferral ledger (recorded, never silent — from critics 8/9, post-6.1)

**Majors (7):** E3 layer classification trusts `[package].name` while scanners trust
directories (adversarial) · E4 timeout class misses `return`/block value positions
(**accidental** — close opportunistically at the next arch-suite touch) · E5 UNC/verbatim
Windows paths mis-resolve to "inside" (adversarial; escape-resolver half undeclared) ·
E6 `rust-toolchain*` checked only at repo root (adversarial) · F1 command class passes any
literal containing one of 37 stopwords (**accidental**+adversarial) · F3 `.env.example`
lines without `=` silently dropped by `parse_env_example` — the only fail-open parse
(**accidental**+adversarial, violates the suite-wide I8 posture) · F4 evidence-locator pin
checks substrings, not which `check_*` a locator calls (accidental+adversarial).

**Minors (13):** E8–E12 (cargo-config discovery skips `target`/`.git` dirs and symlinked
dirs; `terminates_statement` has no char-literal case; dotted `[workspace.dependencies.x]`
header blanked; locator bodies unpinned; byte-string literal assembly) · F5–F12 (timeout
hint-word narrowing; tier class const/static-only; enumerated executable heads; root-table
header exact-match; walker and "independent" listing flatten IO errors in lockstep;
adapter selection by first segment; scope-agreement loops true by construction;
repo-level greens currently evidence over doc-only stub crates).

Rule for these: a deferred finding **upgrades to blocking** the moment a task walks into
its trigger (e.g. F2 was deferred-class until Task 5's defaults module made it live — it
is now closed). Re-litigate E4/F1/F3 first time the arch suite is touched for any reason.

### Deviations recorded this session (for the operator)

1. **`f85c925` was created by the environment's Jolli auto-capture tooling**, not by the
   orchestrator: non-conventional message, no gate/teeth output, and `.jolli/` files in
   the tree against the never-stage-`.jolli` discipline. Left unamended because the Jolli
   system keys its capture records on the SHA. Gate+teeth evidence for round 5 lives in
   this file and the round-6 commit message instead.
2. **`.agents/plans/gauntlet-impl.md:578` carries an uncommitted edit** (upstream
   wayfinder clone URL `github.com/mattpocock/skills`, plus a `../projects/` clone hint)
   made by neither the orchestrator nor the round-6/6.1 builders. Possibly the operator's.
   Left uncommitted and unverified; confirm or drop it before Task 18.
3. The previous harness's staged scratchpad scripts (`teeth3.sh`, `task3-wf.js`) no longer
   exist in this environment; their intent was reconstructed from this file and the plan.

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
| 2 — Workspace skeleton + justfile + gates + boundary test | **done** — closed at round 6.1, residuals recorded above | `b69c095`, `f85c925`, round-6 commit (this session) |
| 3 — Domain types | **in flight** — delegated this session | — |
| 4–27 | not started | — |

### Task 2 — final shape

16-crate workspace (`gauntlet-{domain,ports,app,cli}`, 11 adapters,
`gauntlet-arch-tests`), ten-recipe `justfile` (`ruler` untouched), `.env.example`
(92/92 Annex E vars), `deny.toml` (real policy), 183-test boundary suite: dependency
direction incl. every manifest/config/toolchain channel, per-layer external-dep policy
deny-by-default, source denylist, 13 literal classes over one lexing pass
(`text::regions`: strings/chars/raw/byte/nested comments/escapes), scope-escape
detection (`#[path]`/`include!`/target paths, comment-tolerant), setting-scoped §8.6
defaults exemption at declaration granularity, pinned inventories everywhere, I8-honest
declared build inputs, and the four AK evidence locators under `tests/contract/`.

### Design decisions taken during Task 2 — preserve these

- **Composition-root carve-out.** The adapter-identifier rule (AK-002, §8.1) applies to
  `gauntlet-domain`, `-ports`, `-app` — **not** to `gauntlet-cli`, which spec §4.1 makes the
  composition root and which must be free to name and declare adapter crates. `gauntlet-cli`
  remains subject to the hardcoded-literal scan.
- **`GAUNTLET_RESULT_PATH` is deliberately absent from `.env.example`** (runtime-injected
  per child invocation; listing it would invite operator-stale redirection of a result
  contract). Encoded as a named, justified exclusion in `env_example_complete`.
- **Evidence-locator obligation met:** the four AK-001/002/016/019 locators under
  `tests/contract/` resolve to runnable test targets (`the_manifest_declares_exactly_the_four_answer_key_locators`
  pins them); Task 27 can resolve evidence mechanically.
- **The `versioning` rename is applied** (env vars `GAUNTLET_TOOL_VERSIONING`,
  `GAUNTLET_VERSIONING_STRATEGY`; capability/facet `versioning`). `.env.example` and Annex E
  must always change together (`env_example_complete` diffs them). The spec's §8.6 settings
  table still says `git_strategy` — that matches the frozen spec; do not "fix" one side.
  `crates/adapters/gauntlet-adapter-observe-git` deliberately keeps the product name (§24.3).
  Task 4's `Observer` port uses `Versioning` as the facet-kind variant, never `Git`.

### Operator edits to the spec, absorbed

The operator renamed the `git` tool capability and observation facet to `versioning`
(`Core-spec.md:573`, `:886`). Annex A untouched, `just ruler` unaffected. Task 4 (ports)
and Task 9 (observers) must use `versioning`, not `git`.

### Task 0 outcomes (do not re-derive)

- `.agents/reports/environment.json` + `.md` — full environment fingerprint.
- `.agents/reports/delegation-spike.md` — delegation is admission-based (handle returns
  synchronously, result arrives later); **build Task 4 on the admission-based design as
  specified** (`invoke -> handle`, `poll`, `collect`, result-contract file). Do not re-spike.
- Directory skeleton per spec §25 created; `crates/…` layout owned by Task 2 (done).

### Task 1 outcomes — THE FROZEN RULER

- `.wayfinder/answer-key.md` — spec Annex A verbatim, 70 Units, AK-001…AK-070.
- `.wayfinder/reference.yaml` — manifest wrapper; `.gauntlet/reference.lock` — freeze record
  (answer key `sha256:ccc7c99b…`, manifest `sha256:95331d3e…`).
- `scripts/reference-digest.sh`, `scripts/verify-ruler.sh`, `justfile` (`just ruler`).
- **Design decision Task 7 depends on:** the digest script normalizes CRLF→LF and collapses
  end-of-file newline drift, **nothing else** (per-line trailing whitespace is NOT stripped —
  ruler sensitivity beats the plan's looser wording). Task 7's Rust `normalize` must
  reproduce these exact rules; the two implementations agreeing is the Task 7 gate.

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

**Memory is the real constraint.** Keep `max_parallel` low (spec default 2 is right here);
do not fan out concurrent Rust-compiling builders or you will OOM.

**Containment consequence (recorded, don't re-litigate):** `contained` and `local` tiers
get genuine bwrap containment; `cross_cutting` also runs under bwrap as the strongest
*available* real backend — weaker than spec Annex F's typical container/microVM, honestly
recorded for `docs/CONFORMANCE.md` at Task 27. Not a Task 14 redesign trigger.
`irreversible` gets **no** policy file, by design.

---

## Rules that outrank whatever task you are on

1. **Never edit `.wayfinder/answer-key.md`.** A Unit that seems wrong becomes a dated,
   evidenced entry in `.agents/reports/reference-candidates.md` (create when first needed).
2. **The spec outranks the plan.** On conflict, fix the plan; never the spec.
3. **`just ruler` before and after every task.**
4. **Delegate; do not implement directly** (AGENTS.md §"Delegation", spec §10.7). Every
   builder slice gets an independent **blind** critic: only the artifact + relevant
   answer-key units — never builder reasoning, transcript, or attempt count.
5. **Every task needs its gate at exit 0 AND a demonstrated teeth check** (break the
   mechanism, watch the gate fail, revert, record in the commit message).
6. **Parallel slices get separate git worktrees**, never a shared checkout.
7. **`INCONCLUSIVE` is never `PASS`**, at any layer, ever.
8. **One task, one commit**, with gate + teeth output in the message.
9. Irreversible-tier work, human sign-off, or a genuine blocker → record in
   `.agents/reports/escalations.md`, skip to the next unblocked task, continue.
   **Never downgrade a tier or authority to keep moving.**
10. **Task-2 termination rule (round 5, stands):** a surviving critical whose trigger lies
    outside the stated threat model (lib.rs) is a recorded deferral; inside it reopens —
    except that criticals confined to identified subsystems with purely adversarial
    triggers may be recorded as accepted residuals when the budget rule bites. Accidental-
    trigger criticals always reopen.

---

## Answer-key unit status (spec §26 categories)

- **implemented and proven: none.** No Unit has settled (verification machinery is Task 11).
- **implemented but provisional (4):** **AK-001**, **AK-002**, **AK-016** — mechanisms
  enforced by `just boundaries`, teeth demonstrated, blind-critic-hardened through seven
  rounds; provisional because no `VerificationRecord` exists yet and deferred majors
  remain on record. **AK-019** — `env_example_complete` instrument landed and green;
  same provisional status.
- **proposed (66):** every other Unit, including AK-014 (vacuously true; no adapter
  selection exists yet).
- **blocked: none. · unknown: none.**

---

## Open unknowns (recorded in `.wayfinder/reference.yaml`, not silently resolved)

- **UNK-001** — which containment backend `cross_cutting` runs under in real use, beyond
  this host's bwrap-only environment.
- **UNK-002** — should this repo carry a LICENSE, and which? Not evidenced; not invented.
- **UNK-003** — which concrete `AgentRunner` is `runners.default` in `GAUNTLET.md`.
  Deferred to Task 5 / Tasks 15–16.

## Escalations

**ESC-001 CLOSED** (2026-08-15) — delegation capacity exhausted mid-round; Task 3 was
deliberately NOT implemented inline. The escalation's stated risk was correct and held.
Full context in `.agents/reports/escalations.md`. No escalations currently open.

---

## Next action, concretely

**Task 3 — Domain types** (spec §5.2–§5.3, plan Task 3; gate
`cargo nextest run -p gauntlet-domain && just boundaries`; teeth: constructing
`Unit{blast_radius: irreversible, authority: critic}` fails to compile or returns a typed
error).

- If the delegated builder from this session finished and the work is on disk uncommitted:
  independently re-run the gate + teeth, dispatch the two blind critics (constructibility
  of invalid states; spec fidelity + test vacuity), triage per rule 10, commit.
- If starting fresh: delegate to one builder holding the build lock, then the two critics
  in parallel (read-only, non-compiling), then verify + commit.
- Brief essentials: every §5.2 value object and enum as distinct types; `Outcome`
  three-state with **no** `From<Outcome> for bool` and no `is_ok()`; aggregates
  `Reference`, `Unit`, `Run`, `Iteration`, `RoleInvocation`, `Budget` + the record types;
  constructor invariants returning typed errors (`irreversible ⟹ human`;
  `cross_cutting|irreversible ⟹ authority ≠ critic`; `settled` requires evidence +
  verification refs); unknown enum values are parse errors; round-trip and exhaustive
  tier/authority coupling tests. **Zero new dependencies** (AK-001: the domain layer's
  external allowlist is empty — hand-roll ULID validation as a newtype over a validated
  string; `Digest` carries algo+hex, it does not compute digests; exhaustive enumeration
  over small enum sets instead of property-test crates). Evidence locators named by
  AK-004/006/007 (`tests/contract/answer_key_parse::*`) are Task 7's parser's concern —
  build the domain types so that parser can exist, don't build the parser.
- After Task 3: Task 4 (ports + contract suites), admission-based `AgentRunner`, `Versioning`
  facet kind.
