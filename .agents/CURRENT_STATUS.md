# Current status — HANDOFF DOCUMENT

**Last updated:** 2026-08-17 — **Tasks 2, 3, 4 CLOSED (records below). Task 5 CLOSED
(config + doctor; three blind critics + correction round: 1 critical, 3 major, 9 minor
found — all closed or adjudicated). Next: Task 6 (state/evidence/event-log), brief staged
at `.agents/reports/task6/builder-brief-round1.md`.**
**Read this file first.** Then `AGENTS.md`, `.agents/specs/Core-spec.md`,
`.agents/plans/gauntlet-impl.md` — in that order.

---

## TL;DR for the next agent

Tasks **0, 1 complete** (ruler frozen — never edit `.wayfinder/`). **Task 2 CLOSED**
(`1578dde`): 183/183 arch tests, gates exit 0, residuals are recorded deferrals. **Task 3
CLOSED** (`cb2f4ac`): the full §5.2/§5.3 domain model, zero dependencies, 76/76 tests,
invalid states unrepresentable or typed-error-rejected; confirmation blind critic found
**0 criticals**. **Task 4 CLOSED** (`b27b152`): all 17 ports as traits with typed error
enums, `role_result@v1` envelope + Annex-B payloads with strict-only decoding, 17 shared
contract suites with proven teeth, gates 322/322 workspace; three blind critics, 0
criticals, correction round closed 5 residuals; adversarial critic found **no AK-014
evasion route** through the Annex-B wire-key exemption. **Task 5 CLOSED** (this session):
config resolution (defaults < GAUNTLET.md < env < flags, `.env` under process env),
§8.6 defaults module, canonical serialization + `config_fingerprint`, all eight §8.5
doctor checks, repo `GAUNTLET.md`, `gauntlet doctor` CLI. Critic loop found the round's
critical — **a UTF-8 BOM silently discarded the entire config layer while doctor claimed
"contract found and parsed"** — plus AK-049 spelling/location evasions, io-error
conflation, flow-mapping duplicate-key fingerprint collision, and an explicit-`--config`
NotFound fallback; all closed with teeth, confirmation critic verified 10/11 then the
remainder. Workspace 385/385.

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

### Task 4 closure record (this session, commit `b27b152`)

Recovery + layered closure: a previous session's builder left ~4.4k lines of Task-4 work
uncommitted and uncompiling (3 breaks: `evidence_store.rs:178` delimiter, missing
`contract/stubs.rs` + `suite_runs.rs`, `json.rs` keys signature, `role_result.rs` u64→u32).
Procedure per deviation 4: verify partial state compiles → recon gap reports (two explore
agents) → finisher brief → blind critics → correction round → final doc slice. Workspace
322/322 tests, arch 190/190, ports 56/56; lint/check/deny/ruler all 0/PASS.

**Adjudication worth preserving:** the arch suite's whole-literal adapter-segment rule
(= AK-014's "adapter selection by literal" tripwire) false-fired on the Annex-B.2 wire key
`"command"` in `role_result.rs` (spec-mandated JSON vocabulary, not adapter selection).
Resolution: module+key+position-gated exemption (`ANNEX_B_CONTRACT_PARSER` +
`ANNEX_B_WIRE_KEYS`, key-access positions only), fail-closed across escaped/byte/raw/
padded spellings, corners pinned; **adversarial blind critic verified no evasion route** —
the exemption is "the tightest defensible slice". Its recorded residuals: receiver
invisibility incl. module-path callees (`helpers::get("k")`), array-purpose masking inside
the one module, leading-space spellings all masked. Any future widening is a deliberate
edit to walk.rs's audited inventory.

**E4/F1/F3 re-litigation (ledger-mandated on arch-suite touch): ALL CLOSED**, each with
failing-before evidence (see commit message): E4 timeout class accepts
`return`/block-value positions; F1 stopword exemption yields to command SHAPE (flag token
or shell metachar); F3 `=`-less `.env.example` lines are findings, not skips (the last
fail-open parse). Recorded false-positive shapes: F1 — a prose string citing a flag
(`"git push --force is dangerous"`) flags; E4 — a count returned from a
duration-hint-named fn (`fn timeout_retry_limit() -> u64 { return 5; }`) flags. Both err
toward flagging, by design, documented in literals.rs docs.

**Accepted-with-doc (critic adjudications, 0 criticals):** `IdGenerator` has no error
enum (infallible per §6.2; entropy failure is an adapter concern) · `invoke` has no
deadline parameter (admission is non-blocking by contract; `collect`/`cancel` carry
deadlines) · `RoleInvocationSpec.budget_slice: Budget` (whole budget carried; meter
derives slices) · no vacuous getter cases added for last_offset/hooks/backend_identity ·
8-draw id-collision sampling · probe budget default instant for in-repo teeth runs (real
adapters pass caller-supplied patience). **Deferred to later tasks:** EventLog
fsync/durability + crash-injection (Task 6, per plan); runner capability-vs-tier
dispatch check (Task 14, the `Isolation` ladder is in place).

### Task 5 closure record (this session)

- Builder round 1 + K-correction + M1/M2 micro-correction. Reports:
  `.agents/reports/task5/{builder-round1.md, builder-checkpoints.md, critic-a-result.md,
  critic-b-result.md, critic-c-result.md}` (critic1-spec.md is a partial pre-interruption
  evidence file, superseded).
- **Interruption recovery:** the round-1 builder completed; the session was interrupted
  before critics ran; the M1/M2 agent was cancelled mid-work (code landed, proof missing)
  — recovered by a finisher that only added tests/teeth/report. Both recoveries followed
  the documented pattern: reconstruct state from files, never from conversational memory.
- **Critic yield:** A (spec): 1 major (AK-049 single-spelling probe), 2 minor. B
  (adversarial): 1 critical (BOM → silent empty config layer), 2 major (io-kind
  conflation; flow-mapping duplicate keys → fingerprint collision), 5 minor. C
  (confirmation, post-correction): 10/11 CLOSED, no regressions; the B-R2 remainder
  (explicit `--config` NotFound → silent defaults) closed by M1; unreadable policy dir
  never-clean closed by M2 (both with live CLI teeth and bite-proven tests).
- **Adjudicated, not defects:** skipped-sub-under-check-pass documented in `report.rs`
  (a check's pass relies only on its pass sub-clauses); policy-stem contract matches
  exactly `irreversible` (case-insensitive, any extension, both effective and default
  dirs, `symlink_metadata`) — `irreversible.json.bak`/`.irreversible` are out of contract
  by decision; `max_cost_usd` Money kind (Int|Str) accepts the spec §8.2 example.
- **Recorded-act edits:** `sha2` added to `APPLICATION_EXTERNAL_ALLOWLIST` (config
  fingerprint; reused by Task 7's reference digest) with the pin test updated to the new
  pinned state. AK-049's answer-key evidence locator (`tests/e2e/doctor::…`) cannot exist
  under the escapes rule → first entry in `.agents/reports/reference-candidates.md`
  (dated, cites the real in-crate tests). **The ruler was not touched** — `just ruler`
  PASS verified after every round.
- **Honest unavailables (in doctor.json with reasons):** check 2 deep unit-schema +
  digest-vs-lock (Task 7 parser/`verify_unchanged`); check 3 runner resolution (Tasks
  15–16); check 4 capability-vs-tier (Task 14); check 6 tool binding (Tasks 15–16).
- **Orchestrator verification (independent):** full gate exit 0; focused 62/62;
  workspace 385/385; `just doctor` exit 0 on the repo; BOM tooth, `irreversible.yaml`
  tooth, explicit-missing-`--config` tooth, chmod-000 policy-dir tooth all re-observed
  from the transcripts and spot-probed live.

### Task 3 closure record (this session, commit `cb2f4ac`)

Two-phase build (first builder hit a usage limit mid-task with unwired modules —
"compiles clean" was hollow until lib.rs declared the modules; the finisher caught that,
wired everything, fixed 7 latent defects, added run/iteration/capability). Round 2 closed
both criticals — `RoleId::Custom` grammar bypass and the foreign-run settlement hole
(§5.3.2 "in this run"), found independently by both critics — plus four majors
(§13.5 blocking-forces-fail, AK-055 duplicate-RunId dedup, widened no-bool guard,
irreversible sandbox probe). A micro-fix added three missing public re-exports found by
the orchestrator's outside-the-crate probe, plus a permanent public-API settle test.
Confirmation critic (criticC): **CONFIRM, 0 critical / 2 major / 7 minor.**

**Orchestrator verification:** gates green (`gauntlet-domain` 76/76; check/lint/deny/
boundaries 183/183/ruler all 0); inverted outside-the-crate probe asserting the forbidden
`Unit{irreversible, critic}` constructs FAILED with `Err(IrreversibleRequiresHuman{..})`.

**Task-3 deferral ledger:** evidence locators for AK-003…007 resolve at Task 7 (parser
task) — same provisional-status class as AK-001/002/016/019; `SettlementRecord`
store-binding at Task 13 (SettlementService enforces §16.1; the domain type cannot see
the store); 7 minors in `.agents/reports/task3/criticC-confirmation.md` (settle
from-status table, `DraftUnit` evidence drop, duplicate variation axes counting toward
promotion, `attach_iteration` ordering, source-scan guard evasion routes — `into_bool`/
`fn ok` needles, `pub struct …(pub String)` opacity escape, `*self = Self{…}` run-pin
escape — and settle doc under-listing §16.1 deferrals). Close the guard-evasion minors
opportunistically at the next domain-crate touch.

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
directories (adversarial) · E5 UNC/verbatim Windows paths mis-resolve to "inside"
(adversarial; escape-resolver half undeclared) · E6 `rust-toolchain*` checked only at repo
root (adversarial) · F4 evidence-locator pin checks substrings, not which `check_*` a
locator calls (accidental+adversarial).
**Closed since round 6.1:** E4 (timeout class), F1 (command class), F3 (env
fail-open parse) — all closed during Task 4's arch-suite touch (see Task-4 closure).
**New record (Task 4):** F1/E4 false-positive shapes (prose string citing a flag;
count from a duration-hint-named function) documented in `literals.rs` docs — known,
erring toward flagging.

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
4. **A sub-agent usage limit killed the first Task-3 builder mid-task** (reset 02:39 next
   day). Delegation itself remained available to fresh agents. Recovery was a finisher
   brief; no orchestrator implementation was needed, so no ESC entry. If it recurs, follow
   the "Next action" note on finisher dispatch.
5. **Task 4 started as an orphaned partial state** — a previous session's builder left
   ~4.4k lines of uncompiling ports work uncommitted (3 breaks + 11 missing suites).
   Recovered by the documented procedure (verify partial state → recon gap reports →
   finisher). Lesson learned: check `git status` before assuming a task is unstarted.
6. **Sub-agent final messages were lost FOUR times this session** (3 empty, 1 truncated —
   two `general` builders and two `general` critics). Recovery pattern that worked:
   checkpoint instructions ("append progress to a file after each item") + `explore`-type
   agents for read-only work when `general` keeps losing its tail. The correction builder
   still lost its progress notes; verification had to be re-done from the tree. See
   `.agents/lessons/subagent-message-loss.md`.
7. **(2026-08-17) The session itself was interrupted twice** (harness suspension between
   tool calls). Recovery both times: reconstruct from `git log`/`git status`/report files,
   re-verify the gate personally, then resume the loop at the missing step (critics →
   correction → confirmation). No work was redone that files had already recorded.
8. **(2026-08-17) A cancelled M1/M2 agent left implemented-but-unproven code** (fresh
   mtimes, no report entries). The finisher-brief pattern (prove only, change no
   behavior) closed it in one bounded slice. Checkpoint/report files are the contract;
   the agent's return message is not.

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
| 3 — Domain types | **done** — closed at round 2 + confirmation, residuals recorded above | `cb2f4ac` |
| 4 — Ports + shared contract suites | **done** — closed at `b27b152`, closure record above | `b27b152` |
| 5 — Configuration, defaults, doctor | **done** — closed after 3-critic loop, closure record above | this session |
| 6–27 | not started | — |

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
- **implemented but provisional (8):** **AK-001**, **AK-002**, **AK-016** — mechanisms
  enforced by `just boundaries`, teeth demonstrated, blind-critic-hardened through seven
  rounds; provisional because no `VerificationRecord` exists yet and deferred majors
  remain on record. **AK-019** — `env_example_complete` instrument landed and green;
  same provisional status. (Task 4 did not move these — the new wire-key exemption and
  E4/F1/F3 closures are boundary-suite changes, not Unit settlements.) **Task 5 added
  AK-015** (config precedence, all four layers test-proven), **AK-017** (unknown
  keys/vars collected and reported), **AK-018** (secrets present-but-never-printed,
  canary-proven on every surface), **AK-049** (no irreversible policy, any spelling,
  both dirs, broken symlinks, unreadable dirs) — same provisional class: teeth-proven
  at the instrument level, no VerificationRecord yet; critic A's "partial-here"
  reflects Task-7/14/15-16 dependencies, not open defects.
- **proposed (62):** every other Unit, including AK-014 (vacuously true; no adapter
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

**Task 6 — State store, evidence store, event log** (plan Task 6; spec §16.3, §18.1-18.4;
gate `cargo nextest run -p gauntlet-adapter-state-sqlite && cargo nextest run -p
gauntlet-app state::export::deterministic`). The builder brief is staged at
`.agents/reports/task6/builder-brief-round1.md` — read it, then the contract suites it
points at. Key shape: rusqlite (bundled) EventLog+StateStore with fsync-per-append and
in-repo migrations; content-addressed write-once EvidenceStore (`sha2`, no overwrite API);
app-layer `state::export` determinism tests; a REAL process-abort crash-injection helper;
contract-suite registration per Task 4's existing mechanism; recorded-act allowlist edits
(`rusqlite`, `sha2`) + deny.toml licence additions. Teeth: export determinism, overwrite
refusal, §16.3-order crash exactly-once, boundaries bite on rusqlite outside adapters.

After Task 6: Task 7 (ReferenceProvider — must reproduce `scripts/reference-digest.sh`'s
normalization byte-for-byte; rules recorded under Task 1 outcomes above) or Task 8 (mock
runner) per the dependency graph.
