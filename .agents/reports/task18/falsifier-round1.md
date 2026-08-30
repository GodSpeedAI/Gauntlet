# Task 18 falsifier round 1 — adversarial report

**Target:** `wayfinder-gauntlet` derivation (spec §9.2, Annex D; AK-064/AK-065).
Mechanism under attack: `crates/gauntlet-app/src/formation/mod.rs`
(`check_reference_invariants`), `crates/gauntlet-app/src/reference/validate.rs`
(`validate_answer_key`), CLI `gauntlet reference validate`, and the fixtures
under `tests/fixtures/formation/`.

**Verdict: CONFIRM, with 3 real gaps recorded (severity: major, not blocking —
see justification below). The teeth exist and are real; the parser's
section-heading and reference-marker matching is stricter-than-obvious in
three specific ways that a formation transcript could plausibly hit.**

---

## Attack 1 — mutation-teeth proof (byte-identical restore)

Baseline, before mutation:

```
$ cargo nextest run -p gauntlet-app formation::
...
Summary [0.010s] 6 tests run: 6 passed, 396 skipped
```

Saved `crates/gauntlet-app/src/formation/mod.rs`, sha256:
`04a873b08a480a457328a46a6ac3f32b50a9d30cecb71bd7661c2f3ae333017a`.

Mutation applied (`git diff` unavailable — file is untracked/new for task18 —
so the exact edit is recorded here verbatim):

```diff
-    for bullet in &bullets.not_yet_specified {
-        if !bullet.ids.is_empty() {
+    for bullet in &bullets.not_yet_specified {
+        if false && !bullet.ids.is_empty() {
```

i.e. neutralized the `FogPromoted` check so it can never fire.

RED, as expected:

```
$ cargo nextest run -p gauntlet-app fog_not_in_answer_key
FAIL [0.005s] (1/1) gauntlet-app::formation fog_not_in_answer_key
  stderr: thread 'fog_not_in_answer_key' panicked at
  crates/gauntlet-app/tests/formation.rs:54:10:
  a fog bullet carrying an AK- reference is refused: ()
Summary [0.014s] 1 test run: 0 passed, 1 failed, 401 skipped
```

```
$ cargo nextest run -p gauntlet-app formation::
FAIL formation::tests::fog_carrying_an_ak_reference_is_refused
  panicked at crates/gauntlet-app/src/formation/mod.rs:257:71: fog leak fails: ()
Summary [0.007s] 6 tests run: 5 passed, 1 failed, 396 skipped
```

Both the integration test (AK-065 locator) and the module unit test caught
the mutation. Restored the file from the saved copy:

```
$ cp <saved> crates/gauntlet-app/src/formation/mod.rs
$ sha256sum crates/gauntlet-app/src/formation/mod.rs
04a873b08a480a457328a46a6ac3f32b50a9d30cecb71bd7661c2f3ae333017a
$ diff -q <saved> crates/gauntlet-app/src/formation/mod.rs && echo BYTE-IDENTICAL RESTORE CONFIRMED
BYTE-IDENTICAL RESTORE CONFIRMED
```

GREEN again:

```
$ cargo nextest run -p gauntlet-app formation:: fog_not_in_answer_key
Summary [0.011s] 7 tests run: 7 passed, 395 skipped
```

**Result: attack failed to find a bug — the test genuinely has teeth. The
mechanism holds.**

---

## Attack 2 — bypass probes against the fog/out-of-scope parser

Method: added 9 temporary `#[test]` probes inside
`crates/gauntlet-app/src/formation/mod.rs`'s existing `#[cfg(test)] mod
tests`, ran them with `cargo nextest run ... -- --nocapture`, then removed
them and restored the file byte-identical (same sha256
`04a873b0...` verified again afterward, `diff -q` confirmed empty).

| Probe | Input | Result | Verdict |
|---|---|---|---|
| `→context-only` no space | `- [D1] ... →context-only` | `Ok(())` | correct — trim handles it |
| ASCII arrow leak | fog bullet `[F1] Leaked via ascii arrow. ->  AK-101` | **`Ok(())`** | **GAP — real bug** |
| AK- id in prose, no arrow | `[F1] whether AK-101's scope should widen` | `Ok(())` | by design (structural convention only; documented in IN-FLIGHT.md) |
| Decision, no arrow, not context-only | `[D1] A decision with no arrow at all` | `Err(DecisionWithoutUnit)` | correct |
| `## Not Yet Specified` (case variant) | fog bullet with `→ AK-101` under mis-cased heading | **`Ok(())`** | **GAP — real bug** |
| `### Not yet specified` (extra hash), after a `## Decisions so far` section | fog bullet `→ AK-101` under triple-hash heading | **`Ok(())`** | **GAP — real bug** |
| `### Not yet specified` as the very first section (no prior `current`) | same fog bullet, but no preceding section | `Ok(())` | same root cause as above, bullet dropped (`None` arm) rather than misfiled — still a silent miss |
| Two ids, one dangling | `[D1] ... → AK-101, AK-999` | `Err(DanglingReference { id: "AK-999" })` | correct — specific missing id named |
| Duplicate `## AK-101` block | `CLEAN_ANSWER_KEY` concatenated with itself | `Err(AnswerKeyInvalid { cause: DuplicateBlock { line: 15, ... } })` | correct — schema layer catches it, formation surfaces it as `AnswerKeyInvalid` |

Actual captured output for the three failing cases:

```
PROBE ascii_arrow_leak: Ok(())
PROBE heading_case_variant (## Not Yet Specified): Ok(())
PROBE heading_extra_hash (### Not yet specified): Ok(())
PROBE heading_extra_hash_as_first_section: Ok(())
```

### Root cause (read from `formation/mod.rs`)

1. **ASCII-arrow bypass** — `parse_bullet` only recognizes the Unicode
   character `→` (`after_marker.split_once('→')`). A bullet using ASCII
   `->` never matches, so `ids` comes back empty and the bullet is treated
   as carrying **no** reference at all — silently passing every invariant
   that depends on `!bullet.ids.is_empty()` (`FogPromoted`,
   `OutOfScopePromoted`), and for a Decisions bullet it would instead
   silently trip `DecisionWithoutUnit` (fails safe there, but not in the
   fog/out-of-scope direction).

2. **Heading-casing bypass** — `section_of` does an exact string match
   against `"Not yet specified"` / `"Out of scope"` / `"Decisions so far"`
   after `.trim()`. Any other casing (`"Not Yet Specified"`) returns `None`.
   `parse_sections`' `None` arm is `None => {}` — the bullets under that
   heading are silently dropped, not attributed to any section, so **no
   invariant check ever sees them**.

3. **Heading-depth bypass (most severe)** — `parse_sections` only recognizes
   a heading line via `line.strip_prefix("## ")` (exactly two `#` then one
   space). `### Not yet specified` (three `#`) does not match that prefix at
   all, so the line is neither treated as a heading nor as a bullet — it is
   simply skipped, and **`current` is left unchanged** from whatever section
   preceded it. Concretely: if a `## Decisions so far` section precedes the
   mistyped `### Not yet specified` heading, every bullet under the typo is
   silently re-filed as a **Decisions** bullet. A fog question carrying a
   real `→ AK-101` reference then satisfies `DecisionWithoutUnit` (it has an
   id) and `DanglingReference` (the id exists) trivially — it is accepted
   as a legitimate decision, not merely unchecked. This is a genuinely
   dangerous shape: a single stray `#` character converts a live fog leak
   into what the tool reports as a validated decision.

### Severity assessment

All three are real gaps in the *parser's* robustness, but I am rating them
**major, not blocking**, for two reasons specific to this codebase:

- The map.md convention is explicitly "fixture-grade markdown, not a strict
  format" (module doc comment, `formation/mod.rs` lines 1–17), and the
  IN-FLIGHT.md design contract that governs Slice A's fixture format is the
  single source of truth for the arrow character and heading text — Slice A
  (the skill doc / fixtures) is what actually produces `map.md` in practice,
  not free-form user/LLM authored text. The clean fixtures in
  `tests/fixtures/formation/` all use the canonical `→` character and exact
  heading casing/depth.
- However, this *is* exactly the kind of formatting drift a formation
  session run through an LLM is likely to introduce over time (auto-correct
  swapping `→` for `->`, a heading level typo), and if it happens, the
  currently-shipped tests (`fog_not_in_answer_key`,
  `wayfinder_gauntlet_conformance`) would **not catch it** — they only
  exercise the canonical-format fixtures. There is no test asserting the
  parser rejects or flags an unrecognized heading/arrow shape; it silently
  no-ops instead. That is the "would the shipped tests catch this"
  criterion from the assignment, and the answer is no.

I'm not marking it blocking because it does not defeat the *shipped*
fixture-based contract (clean/fog-leak fixtures both use canonical
formatting, and the CI-relevant tests pass/fail correctly against those), and
because the module's own doc comment already discloses "fixture-grade
markdown, not CommonMark" as an explicit non-goal of strictness. But it is a
real, reproducible loss of the "fog never appears in the answer key"
guarantee under plausible formatting drift, so I'd flag it for a follow-up
task: either (a) make `parse_sections` fail closed on an unrecognized `##`-ish
heading or unrecognized arrow character (reject rather than silently drop/
misfile), or (b) add negative tests pinning today's actual behavior so any
future change to the heading/arrow matching is deliberate.

---

## Attack 3 — CLI `reference validate` directly

```
$ cargo run -q -p gauntlet-cli -- reference validate --path /tmp/does-not-exist-xyz.md
gauntlet reference validate: reading /tmp/does-not-exist-xyz.md failed: No such file or directory (os error 2)
exit:1

$ cargo run -q -p gauntlet-cli -- reference validate --path /tmp/empty-ak.md   # empty file
OK: 0 units
exit:0

$ cargo run -q -p gauntlet-cli -- reference validate --path /tmp/bad-heading-ak.md   # "## AK-1"
gauntlet reference validate: ("## AK-1"): line 1: heading "## AK-1" does not open an AK-NNN block
exit:1

$ cargo run -q -p gauntlet-cli -- reference validate --path /tmp/bad-coupling-ak.md  # blast_radius: irreversible + authority: critic
gauntlet reference validate: block AK-001 ("## AK-001"): the block violates a Unit invariant: Unit tier/authority coupling violated: irreversible requires human authority, got critic (spec 5.3.2, AK-006)
exit:1

$ cargo run -q -p gauntlet-cli -- reference validate --path tests/fixtures/formation/answer-key-fog-leak.md
OK: 3 units
exit:0
```

- Nonexistent path, malformed `AK-1` heading, and the irreversible/critic
  tier-authority coupling violation: all rejected with exit 1 and a non-empty
  error message. **Attack failed to find a bug** — schema validation and the
  §5.3.2 coupling rule are genuinely enforced at the CLI layer.
- Empty file: accepted, exit 0, `OK: 0 units`. This deviates from what the
  assignment brief assumed ("confirm ... non-zero [exit code] ... for every
  rejection case" listed empty file as one). On inspection this is **not a
  bug**: an empty answer key is vacuously well-formed (zero blocks, zero
  failures), and the spec's own Decisions-bullet rule allows an
  all-`context-only` map.md to legitimately produce zero Units. Reported as
  informational, not a gap.
- `answer-key-fog-leak.md` (schema-valid AK-101/AK-102/AK-103, where AK-103
  is the leaked fog answer): **accepted, exit 0, `OK: 3 units`**. This is the
  expected, correct behavior for this attack — it proves `reference validate`
  alone genuinely does **not** catch the fog leak; only
  `formation::check_reference_invariants` (which cross-references map.md)
  does. Confirmed by design, not a gap.

---

## Attack 4 — guardrails and final gates

```
$ git status --porcelain -- .wayfinder/answer-key.md .gauntlet/reference.lock
(empty)
```

Both frozen files untouched throughout.

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Finished `dev` profile [unoptimized] target(s) in 3.18s

$ just boundaries
...
Summary [25.096s] 228 tests run: 228 passed, 0 skipped

$ just ruler
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
```

All green.

## Cleanup verification

- `crates/gauntlet-app/src/formation/mod.rs` restored byte-identical after
  both the mutation attack and the probe attack (sha256
  `04a873b08a480a457328a46a6ac3f32b50a9d30cecb71bd7661c2f3ae333017a`
  before/after both rounds; `diff -q` empty both times).
- No `attack-*.md` fixtures left in `tests/fixtures/formation/` (directory
  contents confirmed unchanged: `answer-key.md`, `answer-key-fog-leak.md`,
  `map.md`, `map-fog-leak.md`, `grilling-transcript.md` only).
- Temp CLI-attack files (`/tmp/empty-ak.md`, `/tmp/bad-heading-ak.md`,
  `/tmp/bad-coupling-ak.md`) deleted.
- `git status --porcelain` after cleanup shows exactly the same task18
  in-flight build artifacts as at session start (formation module, tests,
  fixtures, skill docs, validate.rs, lib.rs/reference/mod.rs/main.rs wiring)
  plus this report file — nothing else changed.

## Summary

- **9 probe attacks + 1 mutation attack + 5 CLI attacks = 15 attacks
  executed.**
- **1 attack (mutation) proves real teeth** — the shipped test genuinely
  fails when the fog-detection logic is weakened.
- **3 attacks found real gaps** (all the same root cause family: the map.md
  section-heading / reference-arrow matcher fails open rather than fails
  closed on unrecognized input): ASCII-arrow bypass, heading-case bypass,
  heading-depth (`###`) bypass. Severity: **major** (not blocking per the
  reasoning above — canonical fixtures are unaffected and the module's own
  docs disclose non-strict parsing as a deliberate scope choice, but no
  shipped test pins this behavior, so a future formatting drift would slip
  through undetected).
- **11 attacks failed to find a bug** — the mechanism held: dangling
  reference detection, duplicate-block detection, tier/authority coupling,
  CLI exit codes for genuine malformation, and the CLI/formation split
  (schema-valid-but-fog-leaking key accepted by `reference validate` alone,
  correctly caught only by `check_reference_invariants`) all behaved exactly
  as designed.
