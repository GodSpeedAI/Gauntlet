# Implementation Plan — Reference-Anchored Gauntlet Runtime (bootstrap to self-hosting)

**Created:** 2026-08-14
**Source of truth:** `.agents/specs/Core-spec.md` (v2). Quote it; do not paraphrase it from memory. Where this plan and the spec disagree, **the spec wins and this plan is corrected.**
**Originating context:** Greenfield. A near-empty repository, a frozen answer key derived from the spec's Annex A, and an operator who intends to run the build itself as a Gauntlet loop under a strong agentic model with sub-agent fan-out.
**Status of the work today:** Nothing implemented. What exists: the specification, this plan, `AGENTS.md`, and access to two upstream artifacts (a wayfinder-class planning skill, a recursive sub-agent harness). Everything below is unbuilt. Be honest about this in every status update: "configured but untested" is a distinct state from "implemented and proven."

---

## 0. How to use this plan (agent operating instructions)

- Execute tasks in the order given. Dependencies are stated per task and drawn in the graph below. Tasks marked *(parallel-safe)* may be delegated concurrently **only in separate worktrees** — parallel writers in one checkout overwrite each other's assumptions.
- **Every task ends with a verification gate.** Do not mark a task done until its gate command exits 0 **and** its teeth check fails when the mechanism is deliberately broken.
- **CORE PRINCIPLE: one ruler, one writer, one loop — nothing that is measured may touch the measuring device.** Concretely: the frozen reference, the answer key, the event log, and the evidence store are never writable by any role that produces work graded against them. Every task in this plan either proves an invariant from the spec with a test that has teeth, or it does not belong here.
- Match surrounding code style. Until code exists, the idioms come from the spec's §4.1 layering and §24.2 reference profile; once `crates/gauntlet-domain` exists, mirror it.
- **The answer key is the spec, not this plan.** Task gates prove *tasks*; `.wayfinder/answer-key.md` proves *the system*. Never edit the answer key to make a gate pass — that is the exact failure mode the system exists to prevent (spec §23.2, G1/G2).
- **Orchestrator restraint:** decompose and delegate. Do not write implementation code yourself while a delegation mechanism is available (spec §10.7). Fan out builders per slice, fan in through result contracts, and dispatch blind critics per §13.2.
- From Task 20 onward the runtime can execute its own loop. Switch to dogfooding: run remaining tasks *through* the Gauntlet, with this plan as the route and the frozen answer key as the ruler.

### Dependency graph

```text
Task 0 recon + toolchain baseline
  └─ Task 1 FREEZE THE RULER (answer key + digest lock)
       └─ Task 2 workspace skeleton + justfile + gates + boundary test
            ├─ Task 3 domain types
            │    └─ Task 4 ports + shared contract suites
            │         ├─ Task 5 config + doctor
            │         │    └─ Task 14 authority gate + sandbox port + tier policies
            │         ├─ Task 6 state store + evidence store + event log
            │         │    ├─ Task 7 ReferenceProvider + freeze/verify        (P1)
            │         │    └─ Task 13 settlement + atomic commit              (P7, P11)
            │         ├─ Task 8 mock AgentRunner + fixtures
            │         │    ├─ Task 9  observers + observation bundle          (P3)
            │         │    ├─ Task 10 comparator + envelope builder          (P2)
            │         │    ├─ Task 11 verifier + evidence binding            (P12)
            │         │    └─ Task 12 controller + budget + diagnosis        (P4, P6)
            │         ├─ Task 15 prime-agent adapter
            │         └─ Task 16 cli-agent adapter                            (P8)
            ├─ Task 17 role artifacts + harness installer   (needs 5, 15 or 16)
            ├─ Task 18 wayfinder-gauntlet skill derivation  (parallel-safe)
            │    └─ Task 19 companion + repo-owned skills   (parallel-safe)
            └─ Task 20 CLI surface + E2E on mock  (needs 7,9,10,11,12,13,14,17)
                 ├─ Task 21 operator surfaces (feature-gated)
                 ├─ Task 22 memory + capability ledger                        (P10)
                 │    └─ Task 23 optimizer + self-improvement gate            (P9)
                 ├─ Task 24 anti-gaming suite G1–G16 + mutation teeth
                 └─ Task 25 E2E real runner + recovery + budget               (P5, P6, P7)
                      └─ Task 26 baselines A/B/C/D
                           └─ Task 27 final conformance + handoff
```

### Global verification gates (must stay green after EVERY task)

```bash
just check            # cargo check --workspace --all-targets
just test             # cargo nextest run --workspace
just lint             # cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --check
just deny             # cargo deny check
just boundaries       # cargo nextest run -p gauntlet-arch-tests    (layering + no-hardcode denylist)
just ruler            # scripts/verify-ruler.sh                     (answer-key digest unchanged)
```

`just ruler` runs after every task from Task 1 onward. If it ever fails, stop: either the ruler moved (an integrity event that invalidates work done since) or the digest tool is wrong. Both are more important than whatever task is in flight.

⟨Rebuild steps if you touched generated schemas or role/skill artifacts before running the gates:⟩

```bash
just schemas          # regenerate docs/schemas/*.json from the typed definitions
just roles-verify     # re-read installed harness role state, report drift
```

---

## Key facts already discovered (do not re-derive)

The repository is empty, so the highest-leverage facts are external. Pin them here rather than re-researching mid-run. Add `file:line` entries as code appears.

| Thing | Location |
|---|---|
| The ruler (verbatim source of `.wayfinder/answer-key.md`) | `.agents/specs/Core-spec.md` → Annex A (70 Units, `AK-001`…`AK-070`) |
| Unit schema, including the tier/authority coupling rule | spec §5.3.2 |
| Why `AgentRunner` is admission-based rather than call-return | spec §0.4 D1 and §6.1 |
| The result-contract mechanism (the fan-in primitive) | spec §10.3; schemas in Annex B.1–B.2 |
| Prime-Agent: `rlm(...)` returns an admission handle (`rlm_child_id`, `name`, `session_dir`, `model`) and **never** the child's answer; results arrive via `agent_message` or files | spec §10.5 and Annex C; upstream `packages/coding-agent/docs/rlm.md` |
| Prime-Agent: kernel runs model-generated code with the worker's OS permissions — a control environment, not a security sandbox | spec Annex C.2; upstream "Trust Model" section |
| Prime-Agent: harness state (prompts, memories, skills, subagent specs) is agent-mutable and session-local, refinable with rollback by id | spec §10.1, Annex C.4 |
| Wayfinder upstream: skill at `skills/engineering/wayfinder/SKILL.md` in the upstream skills repository; companions at `skills/productivity/grilling`, `skills/engineering/{domain-modeling,research,prototype}` | spec Annex D.2 |
| Wayfinder upstream: with no tracker configured it falls back to a local-markdown convention under `.scratch/<effort>/` with `map.md` plus `issues/NN-<slug>.md`, `Status:`/`Type:`/`Blocked by:` lines | spec §9.2 delta table (our variant replaces this with `.wayfinder/` two-file output) |
| Wayfinder semantics that must survive derivation: destination-first, fog-of-war (precise-question test), out-of-scope never graduates, HITL never self-answered | spec §9.2, Annex D.3 |
| Sandbox reality: policy schemas are the least stable input; the real risk is over-permissive default profiles, not weak isolation primitives; `irreversible` has **no** policy file | spec Annex F, §17.3 |
| Non-obvious invariant: `INCONCLUSIVE` is a third state with no boolean conversion; missing evidence never settles | spec §14.4, I8, AK-005 |
| Non-obvious invariant: an agent may raise but never lower a Unit's `blast_radius`/`authority` | spec §5.3.2, AK-046 |
| Proof and anti-gaming obligations (P1–P12, G1–G16) | spec §23.1, §23.2 |

---

## Task 0 — Reconnaissance, toolchain baseline, and approvals *(phase 0 · blocking)*

**Goal:** A written environment fingerprint exists, every tool this plan assumes is either present or explicitly recorded as absent with a fallback, and the operator has approved the reference profile.

**Why this shape:** The spec forbids hardcoding the environment (§8.1). Discovering it once, into a file, is what lets every later task read configuration instead of guessing. Doing this after code exists means retrofitting assumptions.

### Steps

1. Inventory with `rg --files` and targeted probes: OS/kernel, shell, Rust toolchain, `just`, `cargo-nextest`, `cargo-mutants`, `cargo-deny`, `git`, SQLite, available agent harnesses (`prime-agent`, any headless coding CLI), available sandbox backends, and container/VM capability.
2. Record results to `.agents/reports/environment.json` (machine-readable) plus a one-page human summary — file: `.agents/reports/environment.md`.
3. For each missing tool, record: what it was needed for, the fallback, and whether the fallback weakens any spec invariant. A missing sandbox backend weakens `cross_cutting` containment and MUST be recorded as such.
4. Confirm the reference profile (spec §24.2) with the operator, or record the deviation.
5. Create `.gitignore`, `LICENSE` (if applicable), and the directory skeleton from spec §25 — empty directories with `.gitkeep`, no code yet.

### Gate

```bash
test -s .agents/reports/environment.json && \
python3 -c "import json,sys; d=json.load(open('.agents/reports/environment.json')); \
  assert d['tools'] and d['fallbacks'] is not None and d['sandbox']['backends'] is not None"
```

**Done when:** the report names every tool the plan assumes and its status, and deleting any required key from the JSON makes the gate fail.

**Redesign trigger:** No Rust toolchain and no way to install one → switch the reference profile to the strongest typed language actually available, record the substitution in `docs/CONFORMANCE.md`, and re-map every Rust-specific gate. The spec is language-agnostic; only §24.2 assumes Rust.

---

## Task 1 — Freeze the ruler *(phase 0 · blocking · highest priority)*

**Goal:** `.wayfinder/answer-key.md` contains Annex A verbatim, its digest is recorded in `.gauntlet/reference.lock`, and a script can prove at any moment that it has not changed.

**Why this shape:** Everything after this is graded against this file. If it is created *after* implementation starts, the implementation has already influenced the standard — which is exactly invariant I1's failure mode. Freeze first; build second. The shell digest tool here is deliberately primitive so it can exist before any code does, and Task 7 must reproduce its digest exactly.

### Steps

1. Copy spec Annex A verbatim to `.wayfinder/answer-key.md`. Do not reformat, reorder, or "improve" any Unit.
2. Write `.wayfinder/reference.yaml` — the manifest wrapper (spec §5.3.1): id, version, purpose, desired_state, invariants (I1–I12), settlement criteria (spec §26), `unknowns` (start with the sandbox-backend availability question from Task 0), and `source_refs` pointing at the spec.
3. Write `scripts/reference-digest.sh` — normalize (LF endings, strip trailing whitespace, sort nothing) then `sha256`. Document the normalization rules in the script header; Task 7 will re-implement exactly these rules.
4. Write `.gauntlet/reference.lock` with `{answer_key_digest, manifest_digest, algo, frozen_at, spec_source}`.
5. Write `scripts/verify-ruler.sh` — recompute and compare; non-zero on mismatch. Wire it as `just ruler`.
6. Commit these files in their own commit titled `freeze: bootstrap ruler`. Nothing else in that commit.

### Gate

```bash
just ruler && \
grep -c '^## AK-' .wayfinder/answer-key.md | grep -qx 70
```

**Done when:** `just ruler` exits 0; appending a single space to any line of `.wayfinder/answer-key.md` makes it exit non-zero (teeth); and the Unit count matches the spec.

**Redesign trigger:** If a Unit in Annex A turns out to be unimplementable as written, do **not** edit it. Record a `CandidateRevision` in `.agents/reports/reference-candidates.md` with evidence, and continue against the frozen version. Ruler changes are an operator decision, made between runs.

---

## Task 2 — Workspace skeleton, command surface, and boundary enforcement *(phase 1)*

**Goal:** A compiling multi-crate workspace with the layering of spec §4.1, a `just` command surface, and a test that fails the build if the layering is violated.

**Why this shape:** The boundary test is written *before* there is anything to violate it. Retrofitting layering onto an existing tree is the single most common way ports-and-adapters becomes decorative.

### Steps

1. Create the workspace: `crates/gauntlet-domain`, `-ports`, `-app`, `-cli`, `crates/adapters/*` (one crate per adapter family), `crates/gauntlet-arch-tests` — file: `Cargo.toml`.
2. Declare dependency direction in each `Cargo.toml`: `domain` depends on nothing internal; `ports` depends on `domain`; `app` depends on `domain` + `ports`; adapters depend on `domain` + `ports`; `cli` wires everything.
3. Write `crates/gauntlet-arch-tests`: (a) parse workspace manifests and assert the dependency direction; (b) a source denylist asserting that provider/harness/database/browser/sandbox identifiers appear only under `crates/adapters/**` and configuration files; (c) a no-hardcoded-literals scan (absolute paths, model ids, URLs, port numbers) outside the declared defaults module.
4. Write `justfile` with `check`, `test`, `lint`, `deny`, `boundaries`, `ruler`, `ci`, `doctor`, `schemas`, `roles-verify`.
5. Write `.env.example` listing every variable in spec Annex E with empty values, and `deny.toml`.

### Gate

```bash
just check && just lint && just deny && just boundaries && just ruler
```

**Done when:** all gates pass, and adding `sqlite`/`reqwest`/a model-id literal to `crates/gauntlet-domain` makes `just boundaries` fail (teeth check — do this once, observe the failure, revert).

**Redesign trigger:** If the denylist produces constant false positives on legitimate domain vocabulary, narrow it to import statements and string literals rather than deleting it. A weakened boundary test is worse than none because it looks like coverage.

---

## Task 3 — Domain types *(phase 1)*

**Goal:** Every value object and aggregate in spec §5.2–§5.3 exists as a type, with invalid states unrepresentable, and no domain type depends on infrastructure.

**Why this shape:** The enums here (`BlastRadius`, `Authority`, `Outcome`, …) are the execution-safety fields. Modelling them as strings is how a critic silently downgrades a tier later.

### Steps

1. Value objects: `RunId` (ULID), `IterationNo`, `UnitId` (validated pattern), `Digest` (algorithm-carrying), `RoleId`, and every enum in §5.2 — file: `crates/gauntlet-domain/src/values.rs`.
2. `Outcome` as a three-state enum with **no** `From<Outcome> for bool` and no `is_ok()`-style shortcut; provide only explicit matching helpers.
3. Aggregates: `Reference`, `Unit`, `Run`, `Iteration`, `RoleInvocation`, `Budget`, plus the record types (`ObservationBundle`, `ComparisonRecord`, `VerificationRecord`, `SettlementRecord`, `CandidateRevision`) — one module each.
4. Constructor-level invariants: `irreversible ⟹ human`; `cross_cutting|irreversible ⟹ authority ≠ critic`; `settled` requires evidence + verification refs. Return typed errors, never panics.
5. Property tests: Unit parse/serialize round-trips; unknown enum values are errors; tier/authority coupling holds for all generated combinations.

### Gate

```bash
cargo nextest run -p gauntlet-domain && just boundaries
```

**Done when:** the coupling tests pass and constructing a `Unit{blast_radius: irreversible, authority: critic}` fails to compile or returns a typed error (proves AK-006, AK-007 at the domain level).

**Redesign trigger:** If the aggregate invariants become impossible to enforce in constructors because a record is built incrementally, introduce explicit `Draft*` types that convert into validated aggregates — do not relax the invariant.

---

## Task 4 — Ports and shared contract suites *(phase 1)*

**Goal:** All seventeen ports from spec §6 exist as traits with typed errors, and each has a reusable contract suite any adapter can be run against.

**Why this shape:** The contract suite is the mechanism that makes adapter substitution (P8) cheap. Written per-adapter instead of per-port, tests drift and the substitution proof degrades into "it compiled."

### Steps

1. Define traits in `crates/gauntlet-ports/src/*.rs`, one file per port, each with its own error enum.
2. `AgentRunner` exactly as spec §6.1: `invoke → InvocationHandle`, `poll`, `collect`, `cancel`, `capabilities`. **No method returns a role result directly from `invoke`.**
3. Write `crates/gauntlet-ports/src/contract/` — generic suites parameterized over an adapter factory. `AgentRunner`'s suite includes: admission-returns-before-completion; terminal state reported exactly once; missing result document ⇒ `TimedOut`; malformed result ⇒ `Failed{malformed_result}`; mismatched `invocation_id`/envelope digest ⇒ `Failed{contract_mismatch}`; idempotent cancel.
4. Define the `role_result@v1` envelope and payload schemas (Annex B) as typed structs with strict deserialization (unknown fields rejected for settlement-bearing payloads).
5. Add `RunnerCapabilities` and the `isolation` ladder used later by the sandbox capability check.

### Gate

```bash
cargo nextest run -p gauntlet-ports && just boundaries
```

**Done when:** the contract suites compile and run (against a stub that fails them, initially — that is expected and is the teeth check: a suite no adapter can fail is not a suite).

**Redesign trigger:** If a port needs a provider-specific type to be useful, the abstraction is wrong. Add a semantic capability descriptor instead of leaking the type (spec §8.4).

---

## Task 5 — Configuration, defaults, and `gauntlet doctor` *(phase 2)*

**Goal:** Effective configuration resolves `defaults < GAUNTLET.md < env < flags`, is digested onto the run, and `doctor` performs every preflight check in spec §8.5.

**Why this shape:** `doctor` is where "nothing hardcoded" becomes observable. Every check it performs is a class of failure that would otherwise surface halfway through an expensive run.

### Steps

1. One `defaults` module containing the spec §8.6 table — the only place literals are permitted; the boundary test whitelists exactly this module.
2. `GAUNTLET.md` front-matter parser; `$VAR` indirection resolution with presence-validated, never-printed secrets; unknown keys collected and reported, not silently ignored.
3. `EffectiveConfig` with a stable canonical serialization and `config_fingerprint` digest.
4. `doctor`: all eight checks from §8.5, including the two that are easy to skip — the answer key must **not** be inside any builder-writable workspace, and **no** sandbox policy may exist for `irreversible`.
5. Emit `.agents/reports/doctor.json` and a human summary; non-zero exit on any required failure.

### Gate

```bash
cargo nextest run -p gauntlet-app config:: doctor:: && just doctor
```

**Done when:** precedence tests pass in all four layers; placing a file at `.gauntlet/policies/irreversible.json` makes `just doctor` fail (teeth, proves AK-049); and a secret value never appears in `doctor.json` or logs.

**Redesign trigger:** If `GAUNTLET.md` parsing grows conditional branches per key, replace it with a schema-driven deserialization and keep the schema in `docs/schemas/`.

---

## Task 6 — State store, evidence store, event log *(phase 2)*

**Goal:** Durable, append-only run state with content-addressed immutable evidence and deterministic export.

**Why this shape:** Recovery (I11) and atomic settlement (§16.3) both rest on the event log being durable *before* anything else moves. Building settlement first and persistence second guarantees a rewrite.

### Steps

1. `EventLog` adapter: append-only file or table, fsync on append, monotonic offsets, typed events — file: `crates/adapters/state-sqlite/src/event_log.rs`.
2. `StateStore` adapter (embedded SQL): runs, iterations, invocations, residuals, records, payments, escalations, promotions; schema migrations in-repo.
3. `EvidenceStore` adapter: content-addressed filesystem store, write-once, `pin(run_id, ref)`, no overwrite path in the API at all.
4. Deterministic JSONL export (`export_jsonl`) with stable ordering and no timestamps in the digested portion.
5. Run both adapters through the Task 4 contract suites; add a crash-injection test harness (kill between append and apply).

### Gate

```bash
cargo nextest run -p gauntlet-adapter-state-sqlite && \
cargo nextest run -p gauntlet-app state::export::deterministic
```

**Done when:** exporting the same run twice is byte-identical (AK-054); an attempt to overwrite an evidence ref fails to compile or returns an error; crash-injection leaves the log readable and truncation-free.

**Redesign trigger:** If fsync-per-append is unacceptably slow for the observed workload, batch **only** non-settlement events; settlement events remain individually durable. Never trade §16.3's ordering for throughput.

---

## Task 7 — ReferenceProvider: normalize, digest, freeze, verify *(phase 2 · proves P1)*

**Goal:** The runtime resolves, normalizes, digests, freezes, and re-verifies the reference — reproducing Task 1's digest exactly.

**Why this shape:** Two independent implementations of the same normalization (shell in Task 1, typed here) that must agree is a cheap, strong check that normalization is actually deterministic rather than accidentally stable.

### Steps

1. Implement normalization per `scripts/reference-digest.sh`'s documented rules — file: `crates/gauntlet-app/src/reference/normalize.rs`.
2. `freeze(run_id)`: write normalized bytes to the evidence store, pin them, record `ReferenceVersion` on the run.
3. `verify_unchanged()`: called at every iteration boundary and immediately before settlement; mismatch ⇒ terminal `aborted{reference_mutated}` with both digests preserved as evidence.
4. Answer-key parser producing `Unit`s with strict enum validation and per-block error isolation (one malformed block fails that Unit, not the file).
5. Proof test `tests/proofs/p1_reference_integrity`: a builder fixture writes to the answer key mid-run; assert the run aborts, the frozen copy is unchanged, and the frozen copy's path is outside every workspace root.

### Gate

```bash
cargo nextest run -p gauntlet-app reference:: && \
cargo nextest run --test p1_reference_integrity && \
diff <(scripts/reference-digest.sh .wayfinder/answer-key.md) \
     <(cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md)
```

**Done when:** both digest implementations agree; removing the `verify_unchanged` call from the iteration boundary makes P1 fail (teeth); AK-021–AK-025 have passing evidence.

**Redesign trigger:** If the two digest implementations disagree on real content, the normalization rules are under-specified — fix the *rules* in one place and re-derive both, never patch one side to match.

---

## Task 8 — Mock AgentRunner and fixtures *(phase 2)*

**Goal:** A scripted, deterministic `AgentRunner` that can replay success, malformed results, silence, timeouts, contract mismatch, and a reference-mutation attempt.

**Why this shape:** Without it, every orchestration test costs frontier tokens and becomes flaky — and flaky expensive tests get skipped, which is how the loop's own verification erodes first.

### Steps

1. Implement `crates/adapters/agent-mock` driven by fixture files under `tests/fixtures/scenarios/*.yaml`.
2. Scenarios REQUIRED: happy path; malformed result; no result written (timeout); result with wrong `invocation_id`; result with stale `context_envelope_digest`; builder that claims success while producing a failing artifact; builder that attempts to write the answer key; critic that returns `pass` with no evidence.
3. Deterministic clock and seeded id generator adapters, selected by `GAUNTLET_CLOCK` / `GAUNTLET_ID_SEED`.
4. Run the mock through the full `AgentRunner` contract suite — it must pass all of it.

### Gate

```bash
cargo nextest run -p gauntlet-adapter-agent-mock && \
cargo nextest run -p gauntlet-ports contract::agent_runner
```

**Done when:** all eight scenarios drive distinct, asserted runtime states, and the contract suite passes (AK-059).

**Redesign trigger:** If fixtures start encoding orchestration logic rather than agent output, move that logic into the app layer — the mock returns results, it does not decide anything.

---

## Task 9 — Observers and the observation bundle *(phase 3 · proves P3)*

**Goal:** Instrument-produced facets with full provenance, explicit gaps, and a `claims` facet excluded from verification by construction.

### Steps

1. Facet adapters: `filesystem`, `git`, `build`, `test`, plus a generic `command` facet — file: `crates/adapters/observe-*/`.
2. Every facet records instrument name+version, exact command, environment fingerprint, timing, exit status, and a content-addressed evidence ref.
3. Bundle assembly with `observation_digest` over the normalized bundle minus timestamps; `unavailable_evidence` and `observation_failures` are REQUIRED arrays, populated on every probe failure.
4. The `claims_facet` carries `excluded_from_verification: true` and is filtered out at the type level when constructing verification inputs — not by a runtime `if`.
5. Proof test `p3_real_observation` using the lying-builder fixture from Task 8.

### Gate

```bash
cargo nextest run --test p3_real_observation && \
cargo nextest run -p gauntlet-app observation::
```

**Done when:** the lying builder's claim does not influence any verification input, and deleting the `unavailable_evidence` population makes an observation-failure test fail (teeth; AK-028–AK-030).

**Redesign trigger:** If a facet cannot report provenance (a tool with no version and no stable command line), wrap it so the wrapper is the instrument and its own identity is recorded — do not admit an anonymous facet.

---

## Task 10 — Comparator and the envelope builder *(phase 3 · proves P2)*

**Goal:** The runtime builds comparison envelopes from an allowlist; blindness is a structural property of the serialized envelope.

**Why this shape:** Blindness enforced by prompt wording is unverifiable. Enforced by construction, it is a one-line assertion over bytes — and it stays true when someone later adds a "helpful" context field.

### Steps

1. `EnvelopeBuilder` accepting only: frozen reference (or projection), observation bundle (instrument facets), comparison contract, applicable constraints, relevant open residuals — file: `crates/gauntlet-app/src/compare/envelope.rs`.
2. Serialize canonically, digest, and record the digest on both the invocation and the comparison record.
3. Comparator adapters: `deterministic-diff` and `model-backed` (via `AgentRunner`), both emitting `comparison_record@v1`.
4. Deterministic aggregation (spec §13.5): keyed merge, highest severity wins, blocking forces `fail`, disagreement preserved as `disputed`, **no model in the merge path**.
5. Review-mode dispatch: `single`, `fanout`, `adaptive`, `high_assurance`; adaptive skips dimensions already settled conclusively by instruments; budget-truncated dimensions are recorded as `unavailable_evidence`.
6. Proof test `p2_context_isolation` asserting the serialized envelope contains none of the deny-listed fields, plus a teeth variant that injects one.

### Gate

```bash
cargo nextest run --test p2_context_isolation && \
cargo nextest run -p gauntlet-app compare::
```

**Done when:** the deny-list assertion passes, the teeth variant fails as designed, and aggregation is proven deterministic over shuffled input order (AK-026, AK-027, AK-032–AK-034).

**Redesign trigger:** If a dimension genuinely needs builder context to judge (rare — usually it means the reference is under-specified), add the missing information to the *reference*, not to the envelope.

---

## Task 11 — Verifier and evidence binding *(phase 3 · proves P12)*

**Goal:** Deterministic verification first, three-state outcomes, and evidence binding that rejects stale, unresolvable, or wrong-producer evidence.

### Steps

1. Verifier adapters: `command-exit-code`, `test-report`, `file-digest`, `schema-check` — each reporting `is_deterministic()` and its instrument identity.
2. Verifier selection ladder (spec §14.2), cheapest conclusive first; model-backed verification only where no instrument can decide, and flagged as such in the record.
3. Evidence binding checks: resolvable, produced in this run at/after the transform it certifies, producer matches `evidence_required.producer`, locator matches. Failure ⇒ `INCONCLUSIVE` with a typed class.
4. Proof test `p12_missing_is_not_passing` covering every layer: verifier, settlement precondition, and (later) the operator `resolve` action.

### Gate

```bash
cargo nextest run --test p12_missing_is_not_passing && \
cargo nextest run -p gauntlet-app verify::
```

**Done when:** an absent evidence file yields `INCONCLUSIVE` (never `PASS`), stale evidence from a prior iteration is rejected, and removing the staleness check makes a test fail (teeth; AK-035, AK-036).

**Redesign trigger:** If most Units end up needing model-backed verification, the answer key is mis-projected: those claims are quality dimensions, not Units (spec §5.3.2). Record it as a reference candidate revision rather than weakening the verifier.

---

## Task 12 — Controller: selection, budget, diagnosis *(phase 3 · proves P4, P6)*

**Goal:** A controller that selects the next affordable correction, records rejected alternatives, respects the recovery reserve, and diagnoses rather than re-patching.

### Steps

1. `UnitScheduler`: eligibility per spec §11.2, deterministic ordering, slice construction inheriting the highest tier and most restrictive authority.
2. `CorrectionSelector`: score candidates on the ten dimensions of §15.1; reject governance-blocked paths first; record selection **and** rejected alternatives with reasons.
3. `BudgetMeter` adapter: reserve/charge/remaining across all dimensions; `would_breach_reserve` consulted before every dispatch.
4. Non-convergence detection (same residual id N times) routing into bounded diagnosis; diagnostic budget exhaustion ⇒ `INCONCLUSIVE` with quarantined evidence.
5. Proof tests `p4_corrective_loop` (injected defect closed by a routed iteration) and `p6_budget_control` (non-converging task terminates with reserve intact).

### Gate

```bash
cargo nextest run --test p4_corrective_loop --test p6_budget_control && \
cargo nextest run -p gauntlet-app control::
```

**Done when:** both proofs pass on the mock runner; removing the reserve check lets a run spend past the reserve and fails P6 (teeth; AK-037–AK-039, AK-051, AK-052).

**Redesign trigger:** If the selector's scoring becomes unexplainable, reduce it to an explicit priority ordering with recorded tie-breaks. An opaque controller cannot be improved by the optimizer later.

---

## Task 13 — Settlement, atomic commit, single-writer discipline *(phase 3 · proves P7, P11)*

**Goal:** Settlement requires PASS evidence, follows the three-step commit order, survives a crash at every step, and only the runtime writes Unit status.

### Steps

1. `SettlementService` enforcing all five preconditions of spec §16.1 (including reference-digest re-verification).
2. Commit sequence: durable event append → status/evidence write → in-memory advance. Settlement events keyed by `(run_id, unit_id, verification_ref)` so replay is idempotent.
3. Answer-key writer confined to one module; a boundary test asserts no other module writes that path; runtime mounts the answer key read-only into workspaces.
4. Proof tests: `p7_recovery` (kill between each pair of steps; resume completes exactly once) and `p11_critic_is_not_verifier` (comparator `pass` with no verification record does not settle).

### Gate

```bash
cargo nextest run --test p7_recovery --test p11_critic_is_not_verifier && \
cargo nextest run -p gauntlet-app settle::
```

**Done when:** every kill point resumes correctly with exactly one settlement record; reordering the commit steps makes P7 fail (teeth; AK-040–AK-043, AK-062 partially).

**Redesign trigger:** If idempotent replay proves hard because settlement has side effects beyond the store, move those side effects after step 3 and make them retryable — never before step 1.

---

## Task 14 — Authority gate, sandbox port, tier policies *(phase 4 · proves P5)*

**Goal:** Authority decides *whether*, containment decides *how contained*, both before builder process start, with no unsandboxed fallback and no `irreversible` policy.

**Why this shape:** This is spec defect D2 corrected. A gate that runs after the builder's turn is an audit, not a gate.

### Steps

1. `AuthorityGate` in the scheduler: `irreversible` never dispatches; `authority ≠ critic` escalates before any transform; an agent-originated tier or authority *decrease* is rejected and recorded as a governance event.
2. `Sandbox` port + adapters: `none` (tier `contained` only) and at least one real backend from Task 0's inventory; `validate` (dry run) precedes every use.
3. `PolicyProvider` reading `.gauntlet/policies/<tier>.json`; policies versioned in-repo; `irreversible` file absent by design.
4. Wrap the builder invocation from process start; refuse dispatch when `RunnerCapabilities.isolation` is insufficient for the tier.
5. Proof test `p5_hard_gate`: high-quality result violating one hard requirement is rejected; `irreversible` unit never dispatched; sandbox validation failure escalates instead of falling back.

### Gate

```bash
cargo nextest run --test p5_hard_gate && \
cargo nextest run -p gauntlet-app govern:: && just doctor
```

**Done when:** all three sub-assertions pass; deleting the capability check lets a `none`-isolation runner take a `cross_cutting` unit and fails the test (teeth; AK-044–AK-050).

**Redesign trigger:** If no real sandbox backend exists on this host, do not fake containment. Configure `cross_cutting` to refuse dispatch, record the limitation in `docs/CONFORMANCE.md` and in the manifest's `unknowns`, and keep the port so a backend can be added later.

---

## Task 15 — Prime-Agent adapter *(phase 4)*

**Goal:** A real `AgentRunner` that delegates through the harness's recursive sub-agent mechanism and collects results through the result contract.

**Why this shape:** The upstream call returns an admission handle, never the child's answer. Every naive implementation of this loop awaits a result that will never arrive; the contract file is what makes fan-in possible at all.

### Steps

1. Implement `crates/adapters/agent-prime`: render the kernel program of spec Annex C.1, capture `rlm_child_id`/`name`/`session_dir`/`model` into the handle, and return immediately.
2. `poll` reads the result-contract path; a mailbox reply without a valid result file is `Running`, not `Completed`. Respect the deadline; cancel on timeout and preserve `session_dir` as `session_ref`.
3. Report `capabilities()` honestly: `isolation: none` unless externally wrapped, `result_delivery: file`, `supports_native_subagents: true`, `max_depth` from configuration.
4. Guard the harness API surface behind a version probe; a probe mismatch is a `configuration` failure surfaced by `doctor`, not a runtime surprise.
5. Run the adapter through the unmodified `AgentRunner` contract suite.

### Gate

```bash
cargo nextest run -p gauntlet-adapter-agent-prime && \
cargo nextest run -p gauntlet-ports contract::agent_runner -- --adapter prime
```

**Done when:** the contract suite passes unmodified; a child that exits without writing a result yields `TimedOut`; and no code path awaits a value from the delegation call (grep-asserted in the arch tests).

**Redesign trigger:** If the harness is unavailable in this environment, mark it `configured but untested` in `docs/CONFORMANCE.md` and satisfy P8 with the mock plus the CLI adapter from Task 16. Do not stub the adapter into fake success.

---

## Task 16 — Generic CLI agent adapter and substitution proof *(phase 4 · proves P8)*

**Goal:** A second real runner driven entirely by a configured command template, and the conformance suite passing against two adapters.

### Steps

1. Implement `crates/adapters/agent-cli`: command template from `GAUNTLET_CLI_AGENT_CMD` with `{prompt_file}`, `{workspace}`, `{result_path}`, `{model}` placeholders; no vendor name anywhere in the crate.
2. `poll` checks process state **and** the result contract; a clean exit without a valid result is `Failed{missing_result}`, not success.
3. Parameterize the whole proof suite over the runner adapter; add `--adapter` selection to the test harness.
4. Proof test `p8_adapter_substitution` running the full suite against mock + one real adapter.

### Gate

```bash
cargo nextest run --test p8_adapter_substitution && \
just boundaries
```

**Done when:** the same suite passes against two adapters with zero test-body changes; a vendor identifier added to the adapter's public API fails `just boundaries` (teeth; AK-013, AK-014).

**Redesign trigger:** If a test needs adapter-specific setup, move that setup behind the adapter factory. Adapter-specific *assertions* mean the port leaked.

---

## Task 17 — Role artifacts and harness installer *(phase 4)*

**Goal:** Roles are repository files, installed into whichever harness is configured, with drift detection.

### Steps

1. Write `.agents/roles/{orchestrator,builder,critic,verifier,observer,optimizer}.md` with the front matter of spec §10.1, including per-role `context.allow` / `context.deny` and `permissions`.
2. Loader that validates front matter, resolves `$VAR`s, and produces typed `RoleDefinition`s; `context.deny` feeds the envelope builder's allowlist (Task 10) — the file cannot grant a role more context than the builder permits.
3. `gauntlet roles install --harness <id>`: writes into the harness's role surface (subagent specs, agent markdown files, or a prompt directory) and records identifiers in `.gauntlet/roles.lock`.
4. `gauntlet roles verify`: re-reads installed state and reports drift as a `configuration` failure.
5. Critic role bodies per dimension (functional, architecture, security, evidence-completeness) for fan-out.

### Gate

```bash
cargo run -q -p gauntlet-cli -- roles install --harness $GAUNTLET_AGENT_RUNNER && \
cargo run -q -p gauntlet-cli -- roles verify && \
cargo nextest run -p gauntlet-app roles::
```

**Done when:** install is idempotent, `verify` exits 0 immediately after install and non-zero after an out-of-band edit to installed state (teeth; AK-063).

**Redesign trigger:** If a harness has no writable role surface, fall back to injecting the role body into each invocation's prompt and record `enforcement: advisory` on the invocation — visible weakness beats invisible weakness.

---

## Task 18 — Derive and install `wayfinder-gauntlet` *(phase 4 · parallel-safe)*

**Goal:** A repo-owned formation skill that produces `.wayfinder/map.md` plus a schema-valid `.wayfinder/answer-key.md`, with recorded upstream provenance.

**Why this shape:** Formation is where the ruler comes from. A skill that guesses at unresolved questions produces a confident answer key for the wrong system — which the rest of this machinery will then faithfully build.

### Steps

1. Clone upstream into a temporary directory **outside the project tree**; read the wayfinder skill and its local-markdown fallback convention; record commit SHA, licence, retrieval date.
2. Transform per spec §9.2 and Annex D.3 into `.agents/skills/wayfinder-gauntlet/SKILL.md`. Preserve destination-first, fog-of-war (precise-question test), out-of-scope-never-graduates, HITL-never-self-answered, one-decision-per-session. Replace tracker wiring with the two-file output and typed Units.
3. Write `.agents/skills/wayfinder-gauntlet/PROVENANCE.md`; delete the temporary clone.
4. Add an answer-key validator invoked by the skill (reuses the Task 7 parser) so a non-conforming key is rejected at formation time, not at run time.
5. Conformance test `tests/contract/skills::wayfinder_gauntlet_conformance`: given a fixture grilling transcript, the skill's documented procedure yields a map and a key where no fog entry appears in the key, no out-of-scope entry appears in either, and every Unit validates.

### Gate

```bash
cargo nextest run -p gauntlet-app skills::wayfinder && \
cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key.md && \
test -s .agents/skills/wayfinder-gauntlet/PROVENANCE.md
```

**Done when:** the fixture round-trip passes and a fixture with a fog item promoted into the key fails validation (teeth; AK-064, AK-065).

**Redesign trigger:** If upstream is unreachable, implement the skill from spec §9.2 and Annex D.3 alone and record `upstream: unavailable`. The semantics are fully specified there; the clone is a convenience, not a dependency.

---

## Task 19 — Companion and repo-owned skills *(phase 4 · parallel-safe)*

**Goal:** The formation and operating skills the runtime references exist in-repo with provenance and pass a lint.

### Steps

1. Install companions (`grilling`, `domain-modeling`, `research`, `prototype`) under `.agents/skills/`, each with its own `PROVENANCE.md`.
2. Author `gauntlet-run` (operator procedure: doctor → reference → run → supervise escalations → inspect evidence) and per-role skills for anything too long for a role file body.
3. Skill lint: front matter present and valid, no control logic (no settlement, authority, or budget decisions in skill text — those live in the runtime), no secrets, no absolute paths.

### Gate

```bash
cargo nextest run -p gauntlet-app skills::lint
```

**Done when:** every installed skill passes the lint, and a skill containing a settlement decision fails it (teeth).

**Redesign trigger:** If a skill needs to make a governance decision, that decision belongs in the runtime as typed logic. Move it and leave the skill describing *when to invoke*, not *what to permit*.

---

## Task 20 — CLI surface and first full loop on the mock *(phase 5 · milestone: self-hosting begins)*

**Goal:** `gauntlet` exposes the full command set and completes an end-to-end loop against the mock runner, from freeze to settlement to report.

### Steps

1. Implement the commands of spec §20.1; wire the composition root in `crates/gauntlet-cli` (the only place adapters are constructed).
2. Ensure `run` executes the full control path: doctor → freeze → plan slices → dispatch → observe → compare → verify → select → settle → checkpoint → report.
3. Report generator distinguishing implemented-and-proven / provisional / configured-but-untested / proposed / blocked / unknown.
4. E2E test on a seeded bounded task (a tiny target project under `tests/fixtures/target/`) using the mock runner and real instruments.
5. Snapshot test of `--help` output for the whole command tree.

### Gate

```bash
cargo nextest run --test e2e_full_loop_mock --test cli_surface && \
cargo run -q -p gauntlet-cli -- run --dry-run "seeded bounded task" && just ruler
```

**Done when:** the E2E run settles every blocking Unit of the fixture reference with pinned evidence, and the report's status categories are populated from actual records rather than hardcoded (AK-060, AK-061).

**Redesign trigger:** If the composition root starts making policy decisions, push them into the application layer. The CLI wires; it does not decide.

**After this task: switch to dogfooding.** Run the remaining tasks through `gauntlet run`, with this plan as the route and `.wayfinder/answer-key.md` as the ruler. Record each dogfood transition per spec §19.5.

---

## Task 21 — Operator surfaces (feature-gated) *(phase 5 · optional for Core)*

**Goal:** JSON API and a status view that add observability and action without becoming load-bearing.

### Steps

1. Feature-gate the surfaces; the default build is CLI-only and completes a full loop.
2. Read endpoints (`state`, `runs/{id}`, `units/{id}`, `escalated`) and action endpoints (`refresh`, `resolve`, `retry`).
3. Action endpoints call the *same* `SettlementService` path as automatic settlement; no alternate write path exists. Log operator identity on every action.
4. Escalation queue first in any rendered view; then in-flight invocations, completion by tier, budget burn, sandbox denials.

### Gate

```bash
cargo nextest run --features surfaces --test operator_actions_no_bypass && \
cargo build -p gauntlet-cli --no-default-features
```

**Done when:** `resolve` on a Unit lacking PASS evidence is refused with the same error as the automatic path (AK-062), and the no-default-features build still passes the mock E2E (AK-061).

**Redesign trigger:** If a surface needs its own write path for responsiveness, make it enqueue a command the service consumes — never a second writer.

---

## Task 22 — Developmental memory and the capability ledger *(phase 6 · proves P10)*

**Goal:** Raw memory accumulates immediately; capability promotion requires earned evidence.

### Steps

1. Developmental memory: append every attempt, failure, correction, comparison, payment, settlement, recovery — no gating.
2. Capability ledger with `observed | provisional | promoted | revoked` and the tracked dimensions of spec §19.2, including **gate erosion**.
3. Promotion service enforcing minimum settlements under recorded variation, a handled recovery event, non-increasing failure rate, and no gate erosion.
4. Proof test `p10_no_false_learning`: a single successful run leaves status `observed`.

### Gate

```bash
cargo nextest run --test p10_no_false_learning && \
cargo nextest run -p gauntlet-app memory::
```

**Done when:** one success does not promote; three settlements with *identical* variation axes also do not promote (variation must be real — teeth; AK-055).

**Redesign trigger:** If "variation" is unmeasurable for a capability, it is not ready for promotion. Leave it `observed`.

---

## Task 23 — Process optimizer and the self-improvement gate *(phase 6 · proves P9)*

**Goal:** Optimizer proposals are candidates; promotion requires isolated evaluation, held-out comparison, regression non-inferiority, and valid rollback.

### Steps

1. Optimizer role invocation producing `candidate_revision@v1` only; it has no write access to the active loop's configuration or roles.
2. Evaluation harness: run the candidate in an isolated run (`parent_run` set) against held-out and regression task sets; compare protected capabilities for non-inferiority.
3. Promotion gate implementing all eight conditions of spec §19.4; insufficient evidence ⇒ `provisional`, never promoted.
4. Proof test `p9_self_improvement_containment`: a candidate that improves throughput by disabling a verifier is rejected.

### Gate

```bash
cargo nextest run --test p9_self_improvement_containment && \
cargo nextest run -p gauntlet-app promote::
```

**Done when:** the verification-weakening candidate is rejected with a recorded reason; removing the gate-erosion check lets it promote (teeth; AK-056, AK-057).

**Redesign trigger:** If held-out tasks are too few to be meaningful, generate them from prior runs' settled Units rather than lowering the promotion bar.

---

## Task 24 — Anti-gaming suite *(phase 6)*

**Goal:** All sixteen gaming strategies from spec §23.2 have tests, and each test fails when its defense is removed.

### Steps

1. One test per strategy G1–G16 under `tests/antigaming/`, each driven by a mock scenario or fixture.
2. Each test declares the *mechanism* it depends on so the teeth check is mechanical: a manifest of `(test, defended_mechanism, source_location)`.
3. Run `cargo-mutants` scoped to the defended mechanisms; any surviving mutant in a defended path is a suite defect, recorded and fixed.
4. Record results in `.agents/reports/mutation.json`.

### Gate

```bash
cargo nextest run --test-threads 4 antigaming:: && \
cargo mutants --file-list .agents/reports/defended-mechanisms.txt -- --no-default-features && \
python3 -c "import json;d=json.load(open('.agents/reports/mutation.json'));assert d['missed']==0, d"
```

**Done when:** sixteen tests pass and zero mutants survive in defended paths (AK-058, AK-070).

**Redesign trigger:** If a strategy has no mechanical defense (only a prompt), record it explicitly in `docs/CONFORMANCE.md` as advisory and open a reference candidate revision proposing one. Do not let an advisory defense pass as a proven one.

---

## Task 25 — End-to-end with a real runner, recovery, and budget *(phase 7)*

**Goal:** The full loop settles a bounded task using a real agent adapter, real instruments, real containment for its tier, and survives interruption and budget exhaustion.

### Steps

1. Seed a bounded task in the fixture target project whose reference has at least one Unit per tier up to `cross_cutting`, plus one `irreversible` Unit that must escalate rather than dispatch.
2. Run with the real adapter and a real containment backend; preserve the full evidence bundle.
3. Interrupt mid-settlement; resume; assert exactly-once settlement and preserved run identity.
4. Force budget exhaustion on a non-converging Unit; assert clean termination with the recovery reserve intact.
5. Record the run report to `.agents/reports/e2e-real.md`.

### Gate

```bash
cargo nextest run --test e2e_full_loop_real_runner --test e2e_recovery --test e2e_budget && \
just ruler
```

**Done when:** every blocking Unit of the seeded reference settles with pinned instrument evidence; the `irreversible` Unit appears in the escalation queue and was never dispatched (AK-066, plus real-world confirmation of AK-044, AK-047, AK-048).

**Redesign trigger:** If the real runner's latency makes this test impractical for CI, keep it as a scheduled/nightly gate and preserve its evidence — do not delete it. A loop whose own end-to-end proof is skipped has no proof.

---

## Task 26 — Baselines A/B/C/D *(phase 7)*

**Goal:** Measured evidence about whether the Gauntlet earns its cost.

### Steps

1. Implement runner profiles: A builder-only, B builder+self-review, C builder+fresh critic, D full Gauntlet.
2. Run all four on the same bounded tasks; capture held-out correctness, false-completion rate, defect escape, regressions, iterations, token/tool payment, wall-clock, recovery, verification burden, settlement reliability.
3. Write `.agents/reports/baselines.json` and a short honest narrative — **including if D loses**.

### Gate

```bash
cargo run -q -p gauntlet-cli -- improve baseline --profiles A,B,C,D --tasks tests/fixtures/baseline && \
python3 -c "import json;d=json.load(open('.agents/reports/baselines.json'));assert set(d)>= {'A','B','C','D'}"
```

**Done when:** all four profiles have recorded results and the narrative states the payment ratio explicitly (AK-069).

**Redesign trigger:** If D does not beat C by enough to justify its payment, simplify: drop the least load-bearing stage and re-measure. That outcome is a legitimate result of this plan, not a failure of it.

---

## Task 27 — Final conformance and handoff *(phase 7)*

**Goal:** Every AK Unit has a disposition backed by evidence, and the handoff report tells the truth about what is proven.

### Steps

1. Run `gauntlet verify --all` against the frozen answer key; produce a per-Unit disposition with evidence refs.
2. Write `docs/CONFORMANCE.md`: trust posture, sandbox backends and tier mapping, which profile (Core/Extended/Self-improving) is met, and every advisory-only enforcement.
3. Write `.agents/reports/handoff.md` with the six status categories; list every `INCONCLUSIVE` Unit and why.
4. Ensure documentation set from spec §25 exists and no document claims a status ahead of a passing test.
5. Export the run (`gauntlet export`) and pin the evidence bundle.

### Gate

```bash
cargo run -q -p gauntlet-cli -- verify --all --report .agents/reports/conformance.json && \
python3 -c "import json;d=json.load(open('.agents/reports/conformance.json'));\
assert all(u['outcome'] in ('PASS','INCONCLUSIVE','FAIL') for u in d['units']);\
assert not [u for u in d['units'] if u['outcome']=='PASS' and not u['evidence_ref']]" && \
just ci && just ruler
```

**Done when:** no Unit is `PASS` without a resolvable evidence ref; the handoff report's categories match the conformance JSON; `just ruler` still passes, proving the ruler never moved across the entire build.

**Redesign trigger:** None plausible. If this gate cannot pass, the correct action is to report honestly which Units are `INCONCLUSIVE` and why — not to adjust the gate.

---

## Final acceptance checklist (whole plan)

- [ ] Environment fingerprint recorded with fallbacks and their invariant costs *(Task 0)*
- [ ] Ruler frozen; `just ruler` passes and fails on a one-character edit *(Task 1)*
- [ ] Layering enforced mechanically; a deliberate violation fails `just boundaries` *(Task 2)*
- [ ] Domain invariants unrepresentable when invalid; tier/authority coupling proven *(Task 3)*
- [ ] All ports defined with shared contract suites; `invoke` never returns a result *(Task 4)*
- [ ] Config precedence proven; `doctor` fails on an `irreversible` policy file *(Task 5)*
- [ ] Deterministic export; evidence store has no overwrite path; crash-safe log *(Task 6)*
- [ ] P1 reference integrity, with both digest implementations agreeing *(Task 7)*
- [ ] Mock runner covers malformed, silent, mismatched, lying, and mutating scenarios *(Task 8)*
- [ ] P3 real observation; gaps recorded; claims facet excluded structurally *(Task 9)*
- [ ] P2 context isolation, asserted over serialized bytes, with a teeth variant *(Task 10)*
- [ ] P12 missing ≠ passing at every layer; evidence staleness rejected *(Task 11)*
- [ ] P4 corrective loop and P6 budget control; reserve never spent forward *(Task 12)*
- [ ] P7 recovery exactly-once and P11 critic ≠ verifier; single-writer proven *(Task 13)*
- [ ] P5 hard gates; irreversible never dispatched; no unsandboxed fallback *(Task 14)*
- [ ] Prime-Agent adapter passes the unmodified contract suite; nothing awaits a delegation result *(Task 15)*
- [ ] P8 adapter substitution across two runners with no test-body changes *(Task 16)*
- [ ] Roles installed from repo files; drift detected *(Task 17)*
- [ ] `wayfinder-gauntlet` derived with provenance; fog never enters the answer key *(Task 18)*
- [ ] Companion and repo-owned skills lint clean; no governance logic in skills *(Task 19)*
- [ ] Full mock E2E settles with pinned evidence; CLI surface complete *(Task 20)*
- [ ] Operator actions cannot bypass settlement; CLI-only build still completes a loop *(Task 21)*
- [ ] P10 no false learning; identical-variation settlements do not promote *(Task 22)*
- [ ] P9 self-improvement containment; verification-weakening candidate rejected *(Task 23)*
- [ ] G1–G16 covered with zero surviving mutants in defended paths *(Task 24)*
- [ ] Real-runner E2E, recovery, and budget exhaustion all pass with preserved evidence *(Task 25)*
- [ ] Baselines A/B/C/D measured and reported honestly *(Task 26)*
- [ ] Conformance report: no `PASS` without evidence; handoff distinguishes six states *(Task 27)*
- [ ] `just lint`, `just deny`, `just boundaries`, `just ruler` all exit 0
- [ ] `docs/CONFORMANCE.md` reflects reality — no status ahead of a passing test
- [ ] All global gates green; schemas and role/skill artifacts regenerated and verified

## Guardrails (do not violate)

- **Never edit `.wayfinder/answer-key.md` during implementation.** A Unit that seems wrong becomes a recorded `CandidateRevision`, resolved by the operator between runs. This is the plan's single most important rule; violating it invalidates every proof downstream.
- **Do not add a new surface, adapter, or dimension to make a task pass.** Every item here proves specified behavior; extending the system is a separate, referenced decision.
- **Do not let the orchestrator implement.** While delegation is available, decompose and dispatch. If delegation is genuinely unavailable, stop and report `blocked{no_delegation_mechanism}` unless the override variable is explicitly set.
- **Never `await` a result from a delegation call.** Completion is a schema-valid result document at the declared path. A mailbox reply is a signal.
- **The authority gate precedes containment, and both precede builder process start.** Neither substitutes for the other.
- **Never fall back to unsandboxed execution** when policy validation fails. Escalate.
- **No agent lowers a tier or authority.** Operators may raise; nobody lowers.
- **`INCONCLUSIVE` never becomes `PASS`,** at any layer, including operator actions.
- **Keep unrelated cleanup in its own commit.** Ruler freezing (Task 1), skill derivation (Task 18), and companion skill installation (Task 19) each get their own commit with their provenance files.
- **Report provisional work as provisional.** "Configured but untested" is a real status and using it costs nothing; misreporting it costs the trust the whole system exists to produce.