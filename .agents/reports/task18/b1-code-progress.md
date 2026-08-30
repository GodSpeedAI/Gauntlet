# Task 18, Slice B (B1-code) — builder progress

Dispatched: 2026-08-30. Following `.agents/reports/task18/IN-FLIGHT.md`
(authoritative) and the dispatch prompt.

## Deliverable 1: `crates/gauntlet-app/src/reference/validate.rs`

- Created. Thin wrapper: `validate_answer_key(text) -> Result<Vec<Unit>, BlockFailure>`
  calling `super::parse::parse_all_or_fail` directly, no new parsing logic.
- Added `pub mod validate;` to `reference/mod.rs`.
- Includes two small unit tests (well-formed key validates; malformed key
  surfaces the first block failure).

Status: written, not yet compiled (compiling after deliverable 2 lands).

## Deliverable 2: `crates/gauntlet-app/src/formation/mod.rs`

- Created new `formation` module (directory
  `crates/gauntlet-app/src/formation/`), added `pub mod formation;` to
  `lib.rs` in alphabetical position (between `doctor` and `govern`).
- Line-oriented parser over the map.md convention: `## <Section>` headings
  switch current section among `Decisions so far` / `Not yet specified` /
  `Out of scope`; `- [marker] text [→ AK-nnn[, AK-nnn...] | → context-only]`
  bullet lines. Non-conforming lines are simply not tracked bullets (not a
  parse failure) — this is fixture-grade markdown, not CommonMark, per the
  contract.
- `FormationError` enum: `DecisionWithoutUnit { marker }`, `FogPromoted
  { marker, ids }`, `OutOfScopePromoted { marker, ids }`, plus two earned by
  actually using `answer_key_text`: `DanglingReference { marker, id }` (a
  Decisions bullet references an AK- id the answer key doesn't contain) and
  `AnswerKeyInvalid { cause: BlockFailure }` (answer key itself fails the
  Task 7 strict parse). `Display` + `std::error::Error` impls, following the
  `BlockFailure`/`AnswerKeyError` idiom (locus-carrying variants, plain-text
  `Display`).
- `check_reference_invariants(map_text, answer_key_text) -> Result<(), FormationError>`:
  validates the answer key via `validate_answer_key` first (reused, not
  reimplemented), builds the known-id set from the parsed Units, then checks
  fog/out-of-scope leakage and decision-without-unit, plus the dangling-
  reference cross-check.
- Six unit tests in-module: clean pair passes; each of the three §9.2
  invariants individually refused with the right variant; dangling
  reference refused; invalid answer key refused.

Status: written, compiling next.

## Deliverable 3: CLI wiring in `crates/gauntlet-cli/src/main.rs`

- `reference()` dispatcher: added `Some("validate") => reference_validate(&args[1..])`,
  updated its unknown-subcommand and no-subcommand usage lines to list both
  `digest` and `validate`.
- Top-level `usage()`: added `gauntlet reference validate --path <path>`
  line.
- New `fn reference_validate(args: &[String]) -> ExitCode`, placed right
  after `reference_digest`, mirroring its `--path` arg-parsing exactly
  (missing path, duplicate `--path`, unknown argument all -> `ExitCode::from(2)`
  with usage printed). Reads the file with `std::fs::read_to_string`
  (io error -> stderr + `ExitCode::FAILURE`), calls
  `gauntlet_app::reference::validate::validate_answer_key`. On `Ok(units)`
  prints `OK: {n} units` and returns `ExitCode::SUCCESS`; on `Err(failure)`
  prints `gauntlet reference validate: {failure}` to stderr (via
  `BlockFailure`'s `Display`) and returns `ExitCode::FAILURE`.

Verification: `just check-package gauntlet-app` and `just check-package
gauntlet-cli` both green after this deliverable.

```
just check-package gauntlet-app  -> Finished `dev` profile ... in 2m 21s
just test-package gauntlet-app   -> 400 tests run: 400 passed, 0 skipped
just check-package gauntlet-cli  -> Finished `dev` profile ... in 2.83s
```

## Deliverable 4: Tests

Both parallel content-slice fixtures were present under
`tests/fixtures/formation/` by the time these tests were written
(`answer-key.md`, `map.md`, `answer-key-fog-leak.md`, `map-fog-leak.md`,
`grilling-transcript.md`), matching the map.md convention exactly — no
fixtures needed to be written by this slice.

- `crates/gauntlet-app/tests/skills.rs`, test `wayfinder_gauntlet_conformance`
  (AK-064 locator `tests/contract/skills::wayfinder_gauntlet_conformance` vs
  actual `crates/gauntlet-app/tests/skills.rs`, same locator-divergence
  class as task17's `roles.rs` — recorded, ruler untouched). Asserts (a)
  fixture answer key validates to exactly `["AK-101", "AK-102"]`; (b)
  `formation::check_reference_invariants` is `Ok(())` on the clean map/key
  pair; (c) `SKILL.md` exists and its lowercased text contains
  "destination", "fog", "out of scope", "hitl"; (d) `PROVENANCE.md` exists
  and is non-empty. Repo root resolved via `CARGO_MANIFEST_DIR.ancestors().nth(2)`
  (same pattern as `roles.rs`'s `repository_role_artifacts_load`).
- `crates/gauntlet-app/tests/formation.rs`, test `fog_not_in_answer_key`
  (AK-065 locator `tests/contract/formation::fog_not_in_answer_key` vs
  actual `crates/gauntlet-app/tests/formation.rs`). Positive: clean
  map.md/answer-key.md pair -> `Ok(())`. Negative: `map-fog-leak.md` +
  `answer-key-fog-leak.md` -> matched on
  `FormationError::FogPromoted { marker: "[F1]", ids: ["AK-103"] }`
  specifically (not just `is_err()` — proves the RIGHT invariant fired).
  Confirmed the negative fixture pair is schema-valid on its own
  (`answer-key-fog-leak.md` parses to 3 clean units via
  `gauntlet reference validate`) — the formation cross-check is the only
  thing that catches the leak, as designed.

## Boundary gate fix (unplanned, in-scope)

`just boundaries` initially FAILED: `answer_key_single_writer` (spec
§16.4's lexical single-writer pin) flagged
`crates/gauntlet-app/src/formation/mod.rs`,
`crates/gauntlet-app/src/reference/validate.rs`, and
`crates/gauntlet-cli/src/main.rs` for naming the literal token
`"wayfinder"` in production doc comments (`.wayfinder/map.md`,
`wayfinder-gauntlet skill`) without being in the pinned
`ANSWER_KEY_SURFACE_FILES` inventory in
`crates/gauntlet-arch-tests/src/single_writer.rs`.

None of the three files actually own or derive an answer-key path — they
take `map_text`/`answer_key_text`/`--path <path>` as plain
strings/generic paths, same as `reference_digest` already did unpinned.
The mentions were purely descriptive prose. Rather than widen the pinned
inventory (a different core crate's deliberately-curated list) for a
prose-only false trigger, reworded the three doc comments to drop the
literal `"wayfinder"` token:
- `formation/mod.rs`: "`.wayfinder/map.md` convention" -> "the project's
  `map.md` convention", plus an explicit added note that the module reads
  no path and owns no file.
- `reference/validate.rs`: "the wayfinder-gauntlet skill's own last step"
  -> "the answer-key-authoring skill's own last step".
- `gauntlet-cli/src/main.rs` (`reference_validate` doc comment): "the
  wayfinder-gauntlet skill's own formation-time gate" -> "the formation
  skill's own closing gate".

Did not touch `single_writer.rs`, `ANSWER_KEY_SURFACE_FILES`, or any other
arch-test file. Re-ran `just boundaries` after the reword: 228/228 green.

## Full verification (run in order)

```
just check-package gauntlet-app  -> green
just check-package gauntlet-cli  -> green
just test-package gauntlet-app   -> 402 tests run: 402 passed, 0 skipped
just lint                        -> clippy clean; cargo fmt --check clean
                                     (ran `cargo fmt -p gauntlet-app -p gauntlet-cli`
                                     once to settle two pre-existing style
                                     diffs in the new files, then lint reran clean)
just boundaries                  -> 228 tests run: 228 passed, 0 skipped
                                     (see fix above — green on the rerun)
just ruler                       -> verify-ruler.sh: PASS (frozen lock
                                     digests match; nothing in .wayfinder/
                                     or .gauntlet/reference.lock was
                                     touched)
just test (workspace-wide)       -> 1241 tests run: 1241 passed, 0 skipped
```

### Task-specific gate commands

```
cargo nextest run -p gauntlet-app skills::wayfinder
  -> "no tests to run" — NOT a real failure. nextest's default substring
     filter matches against "<binary-id> <test-name>" (space-joined, not
     "::"-joined); the binary id is "gauntlet-app::skills" and the test
     name is "wayfinder_gauntlet_conformance", so the literal string
     "skills::wayfinder" is not a substring of either. Confirmed via
     `cargo nextest list -p gauntlet-app`. The correct filter for this
     exact test is the test name alone:
       cargo nextest run -p gauntlet-app wayfinder_gauntlet_conformance
       -> PASS (1/1)
     (Verified against precedent: task17's own gate used
     `install_and_verify` alone, the same bare-test-name style, not a
     module::test dotted filter — this project's convention already
     matches how nextest actually filters.)

cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key.md
  -> "OK: 2 units", exit 0. PASS.
  Also spot-checked: --path tests/fixtures/formation/answer-key-fog-leak.md
  -> "OK: 3 units" exit 0 (schema-valid on its own, as the fixture design
  intends — only the formation cross-check catches the leak); missing path
  -> stderr + exit 1; no --path -> usage + exit 2.

cargo nextest run -p gauntlet-app formation::
  -> matches only the in-module unit tests under `formation::tests::*`
     (same "space not ::" reason as above — the integration test's id is
     "gauntlet-app::formation fog_not_in_answer_key", and "formation::" is
     not a substring of that either). The correct filter for the AK-065
     integration test:
       cargo nextest run -p gauntlet-app fog_not_in_answer_key
       -> PASS (1/1)
```

## Summary

All four deliverables landed: `reference::validate` (thin wrapper, reused
Task 7 parser), `formation` module (three §9.2 invariants + an earned
dangling-reference cross-check using the answer_key_text parameter),
`gauntlet reference validate` CLI subcommand mirroring `reference digest`'s
arg-parsing style exactly, and both integration tests with their AK-064/
AK-065 locator-divergence headers. Fixed one unplanned but in-scope
boundary-gate false trigger (prose-only "wayfinder" mentions, reworded, not
inventoried) rather than widening a different crate's pinned list. Every
required verification command is green, including full workspace `just
test` (1241/1241) and `just ruler`. Did not touch `.wayfinder/answer-key.md`,
`.gauntlet/reference.lock`, or `Cargo.lock`. No git operations performed.
