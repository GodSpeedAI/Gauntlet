# Task 18 — design-as-built critic report

**Scope:** independent, fresh-context review of the landed (uncommitted)
Task 18 artifacts against Core-spec §9.2 / Annex D, `.wayfinder/answer-key.md`
Units AK-064/AK-065, the plan's Task 18 section, and the orchestrator's
`IN-FLIGHT.md` design contract. Read-only; no fixes applied. All commands
below were executed by this critic, not taken from progress-file claims.

## Verdict

**CONFIRM** against AK-064 and AK-065, with residuals recorded below (0
blocking, 2 major, 3 minor). The skill, provenance record, fixtures,
validator, formation-invariant checker, CLI wiring, and both contract tests
are present, compile clean, pass in full, and hold up under adversarial
probing of the parser's edge cases — with one real, demonstrated gap (RES-02)
in how forgiving the map.md line-recognizer is, and one demonstrated defect
in the plan/IN-FLIGHT-specified nextest gate invocation (RES-01) that
predates this build (identical failure mode already present and accepted at
task17). Neither rises to blocking: the underlying tests exist, pass, and
have real teeth; RES-02 is a robustness gap against off-fixture input, not a
false-pass on any of the required fixtures or the required Done-when
teeth (fixture round-trip passes; fog-promoted fixture fails validation).

## Residuals

### RES-01 — plan/IN-FLIGHT Gate command, run verbatim, errors out (exit 4)

- **dimension:** evidence
- **severity:** major
- **locus:** `.agents/plans/gauntlet-impl.md:592`, `.agents/reports/task18/IN-FLIGHT.md:105`

Repro:
```
$ cargo nextest run -p gauntlet-app skills::wayfinder
...
Starting 0 tests across 4 binaries (402 tests skipped)
Summary [0.001s] 0 tests run: 0 passed, 402 skipped
error: no tests to run
$ echo $?
4
```
nextest's default substring filter matches against `<binary-id><SPACE><test-name>`
(e.g. `gauntlet-app::skills wayfinder_gauntlet_conformance`), not against a
`::`-joined string, so `skills::wayfinder` never matches anything — the
command literally cannot pass as written, regardless of what the test is
named. The correct invocations are:
```
cargo nextest run -p gauntlet-app wayfinder_gauntlet_conformance   # substring, works
cargo nextest run -p gauntlet-app -E 'test(wayfinder_gauntlet_conformance)'  # filterset, works
```
both of which I ran and both pass (1/1).

This is **not new to this build** — I reproduced the identical failure mode
against task17's own plan-specified gate, `cargo nextest run -p gauntlet-app
roles::` (`.agents/plans/gauntlet-impl.md:566`), which also exits 4 with "no
tests to run," and task17 was already CONFIRMed and closed with this
pattern present (`.agents/reports/task17/closure.md` records a related but
distinct "locator-class divergence," not this specific filter-syntax defect,
as accepted). `crates/gauntlet-app/tests/skills.rs:1-6` and
`tests/formation.rs:1-6` both explicitly self-annotate the directory/locator
divergence as an accepted, recorded class — but the filter-syntax break
itself is not called out anywhere and would surprise anyone who copy-pastes
the plan's Gate line expecting it to run the intended test. Given the
pre-existing precedent and that the underlying tests are real, pass, and are
independently invocable, I score this **major** (a real defect in the
project's acceptance-gate wording, silently non-functional as literally
written) rather than **blocking**. Recommend recording it once, in
`reference-candidates.md` or wherever the task17 locator-class entries live,
so future tasks stop reproducing the same broken gate idiom.

### RES-02 — `formation::parse_sections`'s bullet recognizer silently drops non-canonical lines, including a genuine fog-leak

- **dimension:** functional
- **severity:** major
- **locus:** `crates/gauntlet-app/src/formation/mod.rs:173-176` (`parse_sections`), `:194-219` (`parse_bullet`)

`parse_sections` only treats a line as a tracked bullet when, after
`line.trim_start()`, it starts with the **exact** two-byte prefix `"- "`
(dash, single space), and `parse_bullet` only recognizes a marker when the
very next byte is `[`. Any deviation — two spaces after the dash, a tab
after the dash, a marker missing its closing `]` — causes `parse_bullet` to
return `None`, and the bullet is dropped from `Sections` entirely: it is
never checked against any of the three §9.2 invariants. This is a demonstrated,
executed finding (probe crate depending on `gauntlet-app` as a path dep, run
via `cargo run`):

```
fog_malformed_marker_with_leak: Ok(())
```
for input
```
## Not yet specified
- [F1 malformed, still leaks → AK-101
```
— a bullet that genuinely leaks a real answer-key reference out of fog (the
exact violation §9.2 and AK-065 exist to catch) is **silently accepted**
purely because its marker is missing a closing bracket. The same happens for
an extra space after the dash (`-  [F1] ... → AK-101` → also silently
`Ok(())`) and a tab after the dash. Symmetrically, a `Decisions so far`
bullet that lacks any Unit reference and is *not* `context-only` is also
silently un-checked (no `DecisionWithoutUnit`) if its marker is malformed
the same way.

This does **not** affect any of the required fixtures — `map.md`,
`map-fog-leak.md`, `answer-key.md`, `answer-key-fog-leak.md` all use the
canonical `- [D1] ...` form throughout, so `fog_not_in_answer_key`'s teeth
(AK-065) genuinely fire on the fixture as designed — I confirmed the
positive/negative fixture pair independently via `cargo nextest run -p
gauntlet-app formation::` (6/6 pass, including
`fog_carrying_an_ak_reference_is_refused` and the integration test's
pattern-matched `FogPromoted{..}` assertion). But the invariant checker's
real-world reliability is markedly weaker than the module doc comment
implies ("the dangling reference check is real, not decorative" — true when
triggered, but the trigger itself is brittle against exactly the kind of
markdown a human editing `map.md` by hand, or an editor's autoformatter,
would produce). Scored **major**, not **blocking**, because: the plan's
literal Done-when criterion (fixture round-trip passes; fog-promoted fixture
fails validation) is met, and no shipped fixture or test exercises the
un-canonical form. Recommend either (a) treating any `- ` line inside a
tracked section that does *not* parse as a bullet as a hard parse error
(fail closed) rather than silently ignoring it, or (b) relaxing the
recognizer to tolerate the whitespace variants — either closes the gap;
the current "silently drop" behavior is the wrong failure mode for a
gate whose entire purpose is catching leaks.

### RES-03 — `context-only` marker is case-sensitive; a case-mismatch is caught, but via a misleading error path

- **dimension:** functional
- **severity:** minor
- **locus:** `crates/gauntlet-app/src/formation/mod.rs:198-213` (`parse_bullet`)

`trailer == "context-only"` is an exact, case-sensitive match. Probed:
```
mixed_case_context_only ("→ Context-Only"): Err(DanglingReference { marker: "[D1]", id: "Context-Only" })
```
It does fail closed (no invariant slips through) but reports
`DanglingReference` naming `"Context-Only"` as if it were an intended-but-
missing Unit id, which is a confusing diagnostic for what is actually a
typo'd literal. Similarly a double-arrow line (`"→ AK-101 → extra"`) is
folded into a single bogus id `"AK-101 → extra"` and also fails as a
(misleadingly worded) `DanglingReference`. Neither is a false-accept; both
are minor diagnostic-quality issues, not invariant gaps.

### RES-04 — PROVENANCE.md's upstream commit SHA is unverifiable in this review environment

- **dimension:** evidence
- **severity:** minor
- **locus:** `.agents/skills/wayfinder-gauntlet/PROVENANCE.md:5`

The recorded commit SHA (`6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`) against
`github.com/mattpocock/skills` at path `skills/engineering/wayfinder` cannot
be independently confirmed by this critic — no outbound network tool is
available in this review pass. What *can* be, and was, confirmed: the
claimed temp-clone path (`../projects/tmp-wayfinder-clone-task18`, sibling
of the repo) does not exist on disk now, consistent with the claimed
`rm -rf` cleanup step, and nothing from an upstream clone was committed into
the repository (`git status` shows only the expected Task 18 file set). This
is recorded as a limitation of this review, not a finding against the build;
Annex D.1's `upstream: unavailable` fallback was not invoked, so if the SHA
or repo path is wrong that would only be discoverable with network access.

### RES-05 — `formation::check_reference_invariants` performs no marker-uniqueness check across or within sections

- **dimension:** spec-fidelity
- **severity:** minor
- **locus:** `crates/gauntlet-app/src/formation/mod.rs:164-187`

Probed: a marker `[D1]` reused verbatim across `Decisions so far` and `Not
yet specified` sections passes (`Ok(())`), and nothing in §9.2 or Annex D.3
requires marker uniqueness, so this is not a spec violation — recorded only
as a documented boundary of what the checker does and does not enforce, in
case a later task assumes marker uniqueness is guaranteed.

## What checked out clean (with evidence)

- **Spec fidelity of SKILL.md** (`.agents/skills/wayfinder-gauntlet/SKILL.md`):
  read in full. Destination-first (§"Destination first", lines 22-29),
  fog-of-war with the *exact* precise-question test quoted almost verbatim
  from spec §9.2 ("The test for fog versus a resolvable question is whether
  you can state the question precisely right now — not whether you can
  currently answer it", lines 50-52), out-of-scope as non-graduating
  (§"Out of scope", lines 62-71, "it never graduates... It returns only if
  the destination itself is redrawn, and then as a fresh formation effort,
  not a resumption of this one"), and HITL-vs-AFK with the exact "an agent
  that answers its own grilling questions has broken the skill" language
  (lines 73-87, plus the unattended-run fog-not-invention rule). This reads
  as genuine semantic preservation, not keyword stuffing — each section
  restates the upstream constraint in its own words and ties it to a
  concrete consequence (e.g. "a confidently-filled answer key produced this
  way is worse than an incomplete one: it looks settled while encoding a
  guess").
- **Two-file replacement of tracker wiring**: §"The two files" (lines
  97-116) and the `map.md`/Unit-schema worked examples (lines 118-180)
  correctly replace tracker child-issues/labels/native-blocking-edges per
  the §9.2 delta table, using `depends_on` as ordering-only (line 177-179,
  matching Annex D.3's "dependency never confers authority").
  `blast_radius`/`authority` coupling and the `blast_radius: local` default
  rule (lines 169-190) match Annex D.3's "Add" bullet exactly.
- **No control logic embedded** (Annex D.4): SKILL.md documents the
  schema and formation-time field *assignment*, but explicitly disclaims
  runtime decision-making — "This skill does not decide settlement,
  authority elevation, or budget... What happens with those fields at run
  time... is runtime control logic, not something this skill (or any
  skill) implements" (lines 192-196). Confirmed no settlement/dispatch code
  anywhere in the skill text.
- **PROVENANCE.md** (`.agents/skills/wayfinder-gauntlet/PROVENANCE.md`):
  all six Annex D.1 required fields present — upstream repository, upstream
  path, commit SHA, retrieval date, licence, delta summary — plus a
  retrieval-procedure log and an explicit statement that the
  `upstream: unavailable` fallback was not needed. See RES-04 for the one
  caveat (SHA unverifiable offline).
- **Schema-only vs. formation-cross-check separation** (the design's core
  claim): confirmed by direct execution.
  ```
  $ cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key-fog-leak.md
  OK: 3 units
  $ echo $?
  0
  ```
  `reference validate` alone accepts the fog-leak answer key (its AK-103
  block is structurally well-formed in isolation) — only
  `formation::check_reference_invariants`, given both `map-fog-leak.md` and
  `answer-key-fog-leak.md`, rejects it, and does so with the specific
  `FormationError::FogPromoted { marker: "[F1]", ids: ["AK-103"] }` variant.
  This is exactly the separation of concerns the plan and IN-FLIGHT design
  called for.
- **Test teeth**:
  - `crates/gauntlet-app/tests/formation.rs::fog_not_in_answer_key` asserts
    on the *specific* `FormationError::FogPromoted` variant via `match`,
    including both the offending marker and the leaked id list — not merely
    `is_err()`. Real teeth, confirmed by reading the assertion and by
    independently running it (`cargo nextest run -p gauntlet-app
    formation::` → 6/6 pass, including the module's own five unit tests
    covering all three invariants plus dangling-reference and
    invalid-answer-key cases).
  - `crates/gauntlet-app/tests/skills.rs::wayfinder_gauntlet_conformance`
    checks exact expected Unit ids (`["AK-101", "AK-102"]`, not just
    "parses"), a positive invariant-check call, a four-marker
    case-insensitive substring lint on SKILL.md's text (self-documented in
    the test's own comment as "a light content lint, no NLP" — an honest
    limitation, not a hidden gap), and non-emptiness of PROVENANCE.md.
    Substantive, not trivially satisfiable — I confirmed by independently
    running it (1/1 pass) and by reading its assertions in full.
- **Architecture/layering**: `formation/mod.rs` imports only
  `std::fmt` and two sibling modules within `gauntlet-app`
  (`super::reference::parse::BlockFailure`,
  `super::reference::validate::validate_answer_key`) — no adapter, no I/O,
  no path handling (the module's own doc comment states "This module reads
  no path and owns no file"). Consistent with the application layer owning
  no adapter coupling.
- **Guardrails untouched**:
  ```
  $ git diff -- .wayfinder/answer-key.md .gauntlet/reference.lock | wc -l
  0
  $ just ruler
  verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
  ```
- **Full test/lint/boundary sweep, all green**:
  - `just check-package gauntlet-app` — clean
  - `just check-package gauntlet-cli` — clean
  - `just test-package gauntlet-app` — 410/410 pass (0 skipped), including
    `gauntlet-app::skills wayfinder_gauntlet_conformance` and
    `gauntlet-app::formation fog_not_in_answer_key`
  - `just boundaries` — 228/228 pass (0 skipped)
  - `just lint` — clippy clean, `cargo fmt --check` clean
- **CLI wiring** (`crates/gauntlet-cli/src/main.rs`): `reference()`
  dispatcher's `Some("validate") => reference_validate(&args[1..])` (line
  258); `reference_validate` (lines 573-620) mirrors `reference_digest`'s
  `--path` parsing exactly (double-`--path` and missing-`--path` both
  refused with `ExitCode::from(2)`), prints `OK: <n> units` on success and
  the `BlockFailure`'s `Display` to stderr with `ExitCode::FAILURE` on
  error. Both the top-level `usage()` (lines 40-41) and the `reference`
  subcommand usage lines (261-262, 266-267) were updated to document
  `validate`.
- **Diff scope**: `git status --porcelain` shows exactly the expected file
  set for Task 18 (SKILL.md, PROVENANCE.md, five fixtures, validate.rs +
  its `pub mod` line, formation/mod.rs + its `pub mod` line, main.rs's
  dispatcher/usage additions, two new test files, and the orchestrator's
  own status/report bookkeeping) — no stray or unrelated changes.

## Commands run (for reproducibility)

```bash
cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key.md
cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key-fog-leak.md
cargo nextest run -p gauntlet-app skills::wayfinder        # RES-01: exit 4, "no tests to run"
cargo nextest run -p gauntlet-app roles::                  # RES-01 precedent: same failure, task17's own gate
cargo nextest run -p gauntlet-app wayfinder_gauntlet_conformance   # 1/1 pass
cargo nextest run -p gauntlet-app formation::               # 6/6 pass
just check-package gauntlet-app
just check-package gauntlet-cli
just test-package gauntlet-app                              # 410/410 pass
just boundaries                                              # 228/228 pass
just lint                                                     # clean
just ruler                                                    # PASS
git status --porcelain
git diff -- .wayfinder/answer-key.md .gauntlet/reference.lock   # empty
# scratch probe crate (path-dependency on gauntlet-app, run outside the
# project tree) exercising formation::check_reference_invariants edge
# cases: mixed-case context-only, prose AK- substring w/o arrow, double
# arrow, malformed/whitespace-variant markers — see RES-02, RES-03.
```
