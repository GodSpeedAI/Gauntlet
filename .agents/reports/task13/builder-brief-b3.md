# Task 13 / Builder brief B3 — single-writer arch rule + re-litigation record

You are a BUILDER. Read `AGENTS.md` first. B2's service/wiring and B4's
proofs are NOT yours. You touch ONLY: `crates/gauntlet-arch-tests/**` (new
check + entry), its repo-root contract target if the suite's wiring requires
one, and — if genuinely needed for pinning — doctor-side tests in
`gauntlet-app`. Justify every growth against the suite's central-declaration
and anti-vacuity conventions.

## Context

1. `.agents/reports/task13/recon-code.md` §7 (arch-suite structure, scope
   constants, exemption mechanics; the answer-key writing surface — NO
   writer exists besides git).
2. `.agents/reports/task13/recon-spec.md` — §16.4, §9.4, §10.2, §17.5,
   §22.5, §8.5 check 7 (the confinement/enforcement sections).
3. `.agents/reports/task13/residual-adjudication.md` — N4 + "Standing
   obligations riding along".
4. AK-043 (irreversible/human): "The runtime is the sole writer of Unit
   status and evidence fields; a builder or critic write to the answer key
   is prevented or detected."

## Deliverables

**Item 1 — the single-writer / no-writer arch rule.**
A new check module in `gauntlet-arch-tests` (follow the existing module
shape: central scope constants, anti-vacuity guard, fail-closed parsing)
asserting the §16.4 construction: within the CORE crates, no write-shaped
filesystem operation exists in any module that owns or derives the
answer-key path — pin by INVENTORY (the modules that reference the wayfinder
dir name / `AnswerKeySource` are enumerated and asserted to contain no
write-API usage: `std::fs::write`, `File::create`,
`OpenOptions::write(true)/append`, `remove_file`, `remove_dir_all`,
`rename`, `copy`, `hard_link`, `soft_link`), and pin that the provider
module is read-only by construction. The preregistered P5 hazard is literal
overfit: masked spellings (env-injected paths, `format!`-constructed path
strings, type-erased `impl AsRef<Path>` funnels, trait-object indirection)
escape literal scans. Required honesty: run your own bounded
masked-spelling battery against the rule (at minimum: the four spellings
above), record every escape as a RESIDUAL in the check's docs — never claim
the rule catches what it does not. Widen only via deliberate inventory edit
(the suite's standing rule). Wire the check into the suite entry points and
verify `just boundaries` runs it (197 → grown, count pinned).

**Item 2 — the standing re-litigation record (arch-suite touch obligation).**
> **SCOPE SPLIT 2026-08-26:** Item 2 moved to the ORCHESTRATOR (a parallel
> read-only research agent produces the findings; the orchestrator lands
> `arch-relitigation.md`). You do NOT write it. If `arch-relitigation.md`
> exists when you finish Item 1, read it and record any disagreement with
> your own observations in your result contract; otherwise ignore Item 2
> entirely.

**Item 3 — doctor check 7 pin (N4).**
Verify the §8.5 check-7 clause (answer-key path not inside any
builder-writable workspace) is real and non-vacuous in the doctor
implementation; if its test coverage does not exercise the
answer-key-inside-workspace refusal, add the discriminating test. No
behavior change expected — pin what exists; record if the clause proves
weaker than the spec sentence (do NOT widen doctor behavior in this task).

**Item 4 — AK-043 prevention substrate pin.**
The "prevented" legs that already exist by construction: (a) no write API
in any core module reaching the answer key (Item 1's rule now enforces);
(b) builder effect containment (Task-9 workspace containment + Task-8 mock
effect guards) — write ONE in-crate or arch-level pin that a builder-context
write to a path outside its workspace root is refused by the containment
surface the loop actually uses (cite the existing tests; add only if
unpinned). B4 composes the end-to-end g10 proof — do not duplicate it.

## Constraints, checkpoints, gates, contract

Same hard constraints as prior slices (never touch `.wayfinder/`,
`.gauntlet/reference.lock`, `deny.toml`, nextest config, `AGENTS.md`,
`.jolli/`, `.tmp/`, `.agents/prompts/`; no sub-agents; no parallelism
increases; `just ruler` before/after). Checkpoints:
`.agents/reports/task13/builder-b3-checkpoints.md` STARTED/DONE per item.
Result contract: `.agents/reports/task13/builder-b3.md`.

```bash
just ruler
just test-package gauntlet-arch-tests
just boundaries
just check && just lint
```
