# Task 19 correction round 2 (COR3 + COR4) — builder progress

## Summary (result contract)

**Status: DONE. All required gates pass.**

### What changed (single file: `crates/gauntlet-app/src/skills/mod.rs`)

**COR3 (FALS-01 Part B, major).** `scan_control_logic` now checks
`CONTROL_LOGIC_IDENTIFIERS` and `CONTROL_LOGIC_PHRASES` in two separate
passes instead of one chained iterator:

- `CONTROL_LOGIC_IDENTIFIERS` matches are still flagged unconditionally —
  no exemption, as specified (exact API identifiers, narrow true-positive
  signal).
- For each `CONTROL_LOGIC_PHRASES` pattern, the match's byte start position
  on the (lowercased) line is located with `str::find` (not `.contains()`).
  If the substring `"the runtime"` (case-insensitive) is found in the slice
  of the line *before* that match position, the match is treated as
  sanctioned third-person descriptive prose and is not flagged; scanning
  continues to the next pattern / next line exactly as before. No new
  dependency — still the hand-rolled scanner style.
- The module's `## What this lint is not` doc section (top of file) got a
  new paragraph disclosing the exemption: it's a per-line textual
  proximity check, not grammatical subject parsing; it does not catch
  "This skill, not the runtime, settles the unit"; and it can itself be
  defeated by a sentence mentioning "the runtime" earlier in the line for
  an unrelated reason while still making a skill-authored decision later
  in that same line.

**COR4 (FALS-02, minor).** `label_assignment_is_secret`'s left-boundary
check changed from
`!(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')`
to `!bytes[start - 1].is_ascii_alphanumeric()` — exactly the one-line fix
specified. `_` no longer blocks a label match, so `api_key:`,
`secret_key:`, `auth_token:`, `client_secret:` now correctly match their
respective `SECRET_LABELS` entries; `"monkey:"` still correctly fails to
match label `"key"` (the byte before is `n`, alphanumeric).

### Regression tests added (9 new, all in `skills::tests`, all passing)

COR3 (next to the existing COR1 tests):
- `a_runtime_descriptive_sentence_settling_the_unit_is_not_over_rejected` —
  falsifier's verbatim sentence #1, expects `Ok`.
- `a_runtime_descriptive_sentence_authorizing_budget_is_not_over_rejected`
  — falsifier's verbatim sentence #2, expects `Ok`.
- `a_runtime_descriptive_sentence_granting_authority_is_not_over_rejected`
  — falsifier's verbatim sentence #3, expects `Ok`.
- `a_skill_claiming_to_settle_the_unit_without_a_runtime_mention_is_still_refused`
  — the contract's required discriminator test: re-uses the critic's
  original violation sentence (no "the runtime" on the line) and asserts
  it still rejects, proving the fix is the mechanism (proximity to "the
  runtime"), not a widening back toward COR1's pre-fix gap.

COR4 (next to the existing secret tests):
- `a_compound_api_key_label_is_refused`
- `a_compound_secret_key_label_is_refused`
- `a_compound_auth_token_label_is_refused`
- `a_compound_client_secret_label_is_refused`
- `a_word_merely_ending_in_a_label_is_not_flagged_as_a_secret` — the
  `"monkey:"` boundary-purpose regression test (did not previously exist
  as an explicit test; added per contract instruction "add it if not").

No existing test was weakened, deleted, or had its assertion changed. No
conflict was found between any pre-existing test's expectation and either
fix — all 19 pre-existing `skills::tests` entries plus the 4 pre-existing
`gauntlet-app::skills` integration tests still pass unmodified.

### Gate results (run in the order specified)

```
$ just check-package gauntlet-app
Finished `dev` profile [unoptimized] target(s) in 4.77s
-> PASS (clean compile, no warnings surfaced)

$ just test-package gauntlet-app
Summary [0.564s] 442 tests run: 442 passed, 0 skipped
-> PASS. (433 baseline + 9 new = 442, exact match.)
   All 28 `skills::tests::*` entries pass by name, including:
   - a_bare_control_word_in_ordinary_prose_is_not_over_rejected (PASS)
   - a_skill_claiming_to_settle_the_unit_itself_is_refused (PASS)
   - a_skill_instructing_budget_authorization_is_refused (PASS)
   - a_skill_instructing_authority_elevation_is_refused (PASS)
   - a_runtime_descriptive_sentence_settling_the_unit_is_not_over_rejected (PASS)
   - a_runtime_descriptive_sentence_authorizing_budget_is_not_over_rejected (PASS)
   - a_runtime_descriptive_sentence_granting_authority_is_not_over_rejected (PASS)
   - a_skill_claiming_to_settle_the_unit_without_a_runtime_mention_is_still_refused (PASS)
   - a_compound_api_key_label_is_refused (PASS)
   - a_compound_secret_key_label_is_refused (PASS)
   - a_compound_auth_token_label_is_refused (PASS)
   - a_compound_client_secret_label_is_refused (PASS)
   - a_word_merely_ending_in_a_label_is_not_flagged_as_a_secret (PASS)
   - a_secret_shaped_literal_is_refused_and_never_echoed (PASS)
   - an_indirected_value_is_not_flagged_as_a_secret (PASS)
   - a_pem_block_marker_is_refused (PASS)
   - the_wayfinder_gauntlet_front_matter_shape_parses (PASS)
   (plus all remaining pre-existing skills::tests entries — full 28/28.)
   Integration tests `gauntlet-app::skills::lint_rejects_*` and
   `lint_passes_every_installed_skill` and `wayfinder_gauntlet_conformance`
   also all pass (unchanged, not touched by this correction).

$ cargo nextest run -p gauntlet-app skills::lint
Summary [0.009s] 5 tests run: 5 passed, 437 skipped
-> PASS. Matches falsifier-round1.md's own reproduced 5/5 baseline exactly.

$ just boundaries
Summary [18.390s] 228 tests run: 228 passed, 0 skipped
-> PASS. Matches falsifier-round1.md's own reproduced 228/228 baseline
   exactly (includes answer_key_single_writer, PASS).

$ just ruler
verify-ruler.sh: PASS — answer key and manifest digests match the frozen
lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
-> PASS.
```

### Boundary compliance

`git status --porcelain crates/gauntlet-app/src/skills/mod.rs` is the only
tracked-code change made by this correction round. `.wayfinder/answer-key.md`
and `.gauntlet/reference.lock` are untouched (confirmed via
`git status --porcelain` against both paths — empty output). No `SKILL.md`
file, `PROVENANCE.md`, or `crates/gauntlet-arch-tests/src/single_writer.rs`
was touched. Nothing was committed to git.

(Other files showing dirty in `git status` at task start —
`.jolli/jollimemory/*`, `.agents/CURRENT_STATUS.yml`,
`.agents/reports/doctor.json` — are pre-existing/background-process state
unrelated to this task and were not modified by this correction.)

## Verification manual re-check (COR3 exemption logic, worked by hand)

Sentence 1: `"The runtime settles the unit once every blocking residual is
resolved and evidence is verified."` — pattern `"settles the unit"` found
at some offset N; `"the runtime"` occurs at offset 0, i.e. before N ->
exempt -> `Ok`. Matches test result.

Sentence with no runtime mention: `"This skill settles the Unit once its
evidence passes."` — pattern `"settles the unit"` matches; `"the runtime"`
does not occur anywhere on the line, so it cannot occur before the match
position -> not exempt -> `Err(ControlLogicDetected)`. Matches test result.

COR4: `"monkey:"` — label `"key"` found at offset 3; byte before (`'n'`) is
alphanumeric -> boundary violated -> no match -> not flagged. `"api_key:"`
— label `"key"` found after `"api_"`; byte before is `'_'`, not
alphanumeric under the new check -> boundary satisfied -> matches, value
present and not `$`-prefixed -> flagged. Both match observed test results.
