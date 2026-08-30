# Task 18 IN-FLIGHT — wayfinder-gauntlet derivation

**Dispatched:** 2026-08-30, orchestrator (this session).
**Status:** build phase, two parallel builder slices dispatched.

## Design contract (decided by orchestrator before dispatch)

Two parallel slices, disjoint files, no compile conflict (Slice A touches no
Rust/Cargo file):

### Slice A (B1-content) — skill + fixtures, no Rust
- `.agents/skills/wayfinder-gauntlet/SKILL.md` — derived skill (spec §9.2 +
  Annex D.2/D.3). Preserve: destination-first, fog-of-war (precise-question
  test), out-of-scope-never-graduates, HITL-never-self-answered,
  one-decision-per-session. Replace tracker wiring with the two-file output
  (`.wayfinder/map.md`, `.wayfinder/answer-key.md`) and typed Units (§5.3.2).
  Document that the skill's procedure ends by running
  `gauntlet reference validate --path .wayfinder/answer-key.md` before
  declaring formation done (step 4 of the plan: "validator invoked by the
  skill").
- `.agents/skills/wayfinder-gauntlet/PROVENANCE.md` — upstream repo, path,
  commit SHA, retrieval date, licence, delta summary, OR
  `upstream: unavailable` if the clone can't reach the network (spec Annex
  D.1 redesign trigger — this is an accepted, spec-sanctioned outcome, not a
  failure).
- Fixtures under `tests/fixtures/formation/`:
  - `grilling-transcript.md` — a short fixture transcript.
  - `map.md` — fixture map output. Format (orchestrator-specified so Slice B
    can parse it):
    ```
    ## Decisions so far
    - [D1] <text> → AK-101, AK-102
    - [D2] <text> → context-only

    ## Not yet specified
    - [F1] <fog question text>

    ## Out of scope
    - [S1] <out-of-scope text>
    ```
    Fog and out-of-scope bullets carry NO `→ AK-` arrow in the clean fixture.
  - `answer-key.md` — schema-valid fixture answer key (`## AK-NNN` blocks,
    same field shape as `.wayfinder/answer-key.md`'s real entries, e.g.
    `verify_by: test`, `blast_radius: local`, `authority: critic`,
    `evidence_required: {kind: test_report, locator: ..., producer:
    instrument}`) containing AK-101 and AK-102 (referenced by map.md's D1)
    and nothing else. Must parse cleanly via the Task 7 parser.
  - `map-fog-leak.md` — same as `map.md` but `[F1]` gains `→ AK-103`.
  - `answer-key-fog-leak.md` — `answer-key.md` plus one extra well-formed
    `## AK-103` block (structurally valid on its own — the point is that
    schema validity alone must NOT catch this; only the formation
    cross-check catches it).

### Slice B (B1-code) — Rust: validator + formation check + CLI + tests
- `crates/gauntlet-app/src/reference/validate.rs`: `pub fn
  validate_answer_key(text: &str) -> Result<Vec<gauntlet_domain::Unit>,
  super::parse::BlockFailure>` — thin wrapper over
  `parse::parse_all_or_fail` (reuse, not reimplement). Export via
  `reference/mod.rs`'s `pub mod validate;`.
- `crates/gauntlet-app/src/formation/mod.rs` (new top-level module, add
  `pub mod formation;` to `lib.rs`): parses the map.md convention above and
  checks the three §9.2 invariants:
  1. every `Decisions so far` bullet has ≥1 `AK-` reference or literal
     `context-only`;
  2. no `Not yet specified` bullet carries an `AK-` reference (fog leak);
  3. no `Out of scope` bullet carries an `AK-` reference.
  Typed `FormationError` enum, one variant per invariant, each carrying the
  offending bullet's marker (`[F1]` etc.) as the failure locus. Public fn
  signature: `pub fn check_reference_invariants(map_text: &str,
  answer_key_text: &str) -> Result<(), FormationError>` (answer_key_text is
  accepted for future use / symmetry even if the current checks only need
  map_text — keep the signature; do not add unused-arg warnings, prefix
  `_answer_key_text` only if truly unused, but prefer actually using it to
  double check every `AK-` id in a Decisions/fog/out-of-scope reference
  exists in the parsed answer key too).
- CLI: `crates/gauntlet-cli/src/main.rs` — extend `reference()` dispatcher
  with `Some("validate") => reference_validate(&args[1..])` (mirror
  `reference_digest`'s `--path` arg-parsing style). On success print `OK:
  <n> units` and exit 0; on failure print the `BlockFailure` and exit
  `ExitCode::FAILURE`. Update `usage()` strings (both the top-level and the
  `reference` subcommand usage lines).
- Tests:
  - `crates/gauntlet-app/tests/skills.rs`, module path `skills`, test fn
    `wayfinder_gauntlet_conformance` (AK-064 locator
    `tests/contract/skills::wayfinder_gauntlet_conformance` — locator
    divergence recorded same class as task17's `roles.rs`, never a reason to
    touch the ruler). Loads the Slice A fixtures, asserts:
    (a) `answer-key.md` validates cleanly and yields exactly the expected
    Units;
    (b) `formation::check_reference_invariants` returns `Ok(())` on the
    clean map/key pair;
    (c) `.agents/skills/wayfinder-gauntlet/SKILL.md` exists and its text
    contains the four preserved-semantics markers (case-insensitive
    substring check is fine: "destination", "fog", "out of scope", "HITL");
    (d) `PROVENANCE.md` is non-empty.
  - `crates/gauntlet-app/tests/formation.rs`, module `formation`, test fn
    `fog_not_in_answer_key` (AK-065 locator
    `tests/contract/formation::fog_not_in_answer_key`): positive case (clean
    fixtures) → `Ok(())`; negative case (`map-fog-leak.md` +
    `answer-key-fog-leak.md`) → `Err(FormationError::FogPromoted { .. })`.
    This is the required teeth (plan Done-when).

## Gate (task18, run by orchestrator after both slices land)
```bash
cargo nextest run -p gauntlet-app skills::wayfinder && \
cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key.md && \
test -s .agents/skills/wayfinder-gauntlet/PROVENANCE.md
```
Plus `cargo nextest run -p gauntlet-app formation::` for the AK-065 teeth,
then the global gates (`just check`, `just test`, `just lint`, `just deny`,
`just boundaries`, `just ruler`).

## Recovery note
If this session dies mid-flight: `git status` for landed slice work,
re-read this file for the contract, check `.agents/reports/task18/*-progress.md`
for slice status, do not re-dispatch a slice whose files already exist and
compile/pass — finish the remainder per the Finisher procedure
(`.agents/lessons/session-interruption-recovery.md`).
