# Falsifier round 2 — Task 19 (companion + repo-owned skills, post-COR3/COR4)

**Verdict: CONFIRM** (0 blocking, 0 major, 0 minor — no new residuals found;
all reproduced attacks either behave exactly as the corrected mechanism's
own documentation discloses, or hold as a genuine defense)

Fresh-context, blind, EXECUTING adversarial round against the
COR3/COR4-corrected tree. Did not read any `*-progress.md` file. Read
`critic-design.md`, `falsifier-round1.md`, and `correction-round2.md` for
context on what was already found and corrected; did not check anyone's
homework — independently re-attacked the current `crates/gauntlet-app/src/skills/mod.rs`
by executing real inputs through `lint_skill_text` / `lint_installed_skills`
via a throwaway integration-test file
(`crates/gauntlet-app/tests/zz_falsifier2_attack.rs`) and a throwaway scratch
skills-root directory under the session scratchpad. Both removed after the
run; `git status --porcelain crates/ tests/ .agents/skills/` and
`git diff --cached --stat` (below) confirm the tree matches the pre-existing
task19 changeset exactly — nothing stray added or removed.

## Gate reproduction (run independently)

```
$ cargo nextest run -p gauntlet-app skills::lint
Summary [0.009s] 5 tests run: 5 passed, 437 skipped

$ just boundaries
Summary [18.426s] 228 tests run: 228 passed, 0 skipped
(includes answer_key_single_writer, PASS)

$ just ruler
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock

$ just test-package gauntlet-app
Summary [0.662s] 442 tests run: 442 passed, 0 skipped
```

442 (up from round 1's 433 — the 9 new COR3/COR4 regression tests) all pass.
No discrepancy found in gate reproducibility.

## Attacks executed

### 1. COR3 runtime-exemption — direct exploitation attempts

**1a. "Unlike the runtime, X" / "not the runtime but X" gaming.** Executed
three variants where "the runtime" appears earlier on the line in a clause
that explicitly DENIES the runtime is doing the action, while the skill
itself makes the real decision:

```rust
lint_skill_text("x", doc("Unlike the runtime, this skill authorizes additional budget directly.\n"))
// -> Ok(...)   EXEMPTED

lint_skill_text("x", doc("Unlike the runtime, this skill grants the unit authority on its own.\n"))
// -> Ok(...)   EXEMPTED

lint_skill_text("x", doc("It is not the runtime but this skill that settles the unit here.\n"))
// -> Ok(...)   EXEMPTED
```

Observed output (verbatim):
```
R2-1a unlike_the_runtime_denial: Ok(SkillFrontMatter { name: "x", description: "d", disable_model_invocation: None })
R2-1a2 unlike_the_runtime_grants_authority: Ok(SkillFrontMatter { name: "x", description: "d", disable_model_invocation: None })
R2-1a3 not_the_runtime_settles: Ok(SkillFrontMatter { name: "x", description: "d", disable_model_invocation: None })
```

**Attack executed, gap technically found — but this is the exact, named,
accepted-out-of-scope case, not a new residual.** `correction-round2.md`'s
COR3 fix description states verbatim: "it will not catch a deliberately
adversarial sentence like 'This skill, not the runtime, settles the unit'
(that remains the already-disclosed RES-01/'what this lint is not'
residual; do not chase it further, do not add subject-tracking NLP)." My
`R2-1a3` reproduction ("It is not the runtime but this skill that settles
the unit here.") is a direct paraphrase of that exact named example. The
module doc's "## What this lint is not" section (mod.rs:86-93) also states
this plainly: the exemption "can itself be defeated by a sentence that
mentions 'the runtime' earlier in the line for an unrelated reason while
still making a skill-authored settlement/authority/budget decision later in
that same line." **One nuance worth recording (not filed as a residual,
since it does not change the accepted scope):** the disclosure's wording
("for an unrelated reason") undersells how easy the bypass is in practice —
`R2-1a`/`R2-1a2` show a *directly contradicting* clause ("Unlike the
runtime...") works just as well as an unrelated one, and reads as more
natural adversarial prose than an incidental mention would. This is still
squarely inside the documented "textual proximity, not grammatical subject
parsing" limitation and inside the same disclosed RES-01 residual the
correction contract explicitly declined to chase further — **CONFIRM, not a
new finding**, but noted here for the record since the task brief asked for
this exact attack to be executed and reported honestly either way.

**1b. Same-line-only boundary — split across two lines.** Executed:

```rust
lint_skill_text("x", doc("Consider the runtime carefully.\nThis skill authorizes additional budget for itself.\n"))
```

Observed output (verbatim):
```
R2-1b split_across_lines: Err(ControlLogicDetected { pattern: "authorizes additional budget", line: 6 })
```

**Attack executed and repelled.** Putting "the runtime" on the prior line
does NOT exempt the decisive phrase on the next line — the exemption is
correctly implemented as same-line-only, exactly as COR3 specified
("earlier on that SAME line"). No cross-line bypass found.

### 2. FALS-01 Part A residual — re-attempted, new phrasings added

Re-ran the four round-1 under-rejection sentences verbatim, plus two new
phrasings:

```
R2-2-old close_this_unit: Ok(...)         ("The skill should close this Unit once review finishes.")
R2-2-old bump_budget: Ok(...)             ("Bump the budget ceiling up before continuing the run.")
R2-2-old treat_as_authority: Ok(...)      ("Treat this agent as having lead_agent authority for the rest of the run.")
R2-2-old outcome_is_pass: Ok(...)         ("If tests are green, the Outcome is Pass for this unit.")
R2-2-new finalize_the_unit: Ok(...)       ("Finalize the unit as complete and move on to the next task.")
R2-2-new raise_the_ceiling: Ok(...)       ("Raise the spending ceiling for this run before proceeding.")
```

**Attack executed, residual confirmed still real and honestly disclosed**
— every one of these evades detection, exactly as the module's own "## What
this lint is not" section claims ("a settlement, authority, or budget
decision phrased in yet another way... can still evade it entirely — this
is a disclosed, accepted residual, not an oversight"). Two new phrasings,
invented independently of round 1's wording, took under a minute each to
construct — the disclosure's honesty about ease-of-evasion is not
overclaiming. Not filed as a residual (this is RES-01's already-accepted
scope, per the task brief's own framing of this item as confirmatory, not
novel).

### 3. FALS-02 fix re-verification + new secret-scan gap hunt

**Regression check — the four round-1 reproductions:**

```
R2-3-cor4regress api_key: Err(SecretLikeValueDetected { line: 5 })
R2-3-cor4regress secret_key: Err(SecretLikeValueDetected { line: 5 })
R2-3-cor4regress auth_token: Err(SecretLikeValueDetected { line: 5 })
R2-3-cor4regress client_secret: Err(SecretLikeValueDetected { line: 5 })
```

All four now correctly reject — **COR4 fix confirmed working.**

**New compound-convention / boundary-character attacks:**

```
R2-3-new db_password: Err(SecretLikeValueDetected { line: 5 })       ("db_password: ...")
R2-3-new stripe_secret: Err(SecretLikeValueDetected { line: 5 })     ("stripe_secret: ...")
R2-3-new api-key-hyphen: Err(SecretLikeValueDetected { line: 5 })    ("api-key: ..." — hyphen before label)
R2-3-new extra_whitespace: Err(SecretLikeValueDetected { line: 5 })  ("api_key:    ...")
R2-3-new tab_whitespace: Err(SecretLikeValueDetected { line: 5 })    ("api_key:\t...")
```

**Attack executed and repelled on all five.** No new compound-label
convention, hyphen-boundary variant, or whitespace-shape variant bypasses
the fixed boundary check — `!bytes[start-1].is_ascii_alphanumeric()`
correctly treats `_` and `-` alike as non-blocking, and `line[end+1..].trim_start()`
absorbs any amount/kind of whitespace before the value.

**False-positive risk check (per spec §8.1's actual indirection
convention, "Config values may reference `$VAR`" — bare `$`-prefix is the
ONLY sanctioned form, nothing else):**

```
R2-3-fp env_var_braces: Ok(...)      ("token: ${API_TOKEN}")  — starts with '$', exempted
R2-3-fp env_function: Err(SecretLikeValueDetected)  ("token: env(API_TOKEN)")
R2-3-fp vault_ref: Err(SecretLikeValueDetected)     ("secret: vault://path/to/secret")
```

`env(API_TOKEN)` and `vault://...` are flagged even though they read as
"clearly an indirection reference" in general parlance. This is **not a
false-positive bug**: spec §8.1 defines exactly one sanctioned indirection
form (`$VAR`), and IN-FLIGHT.md's schema is deliberately closed/narrow
("secrets by indirection only... Config values may reference `$VAR`"). A
skill author using a non-`$` indirection convention is correctly forced to
rewrite it in the one form this project recognizes — that is the intended,
designed behavior, not a defect. No new gap found in either direction
(under- or over-rejection) for the secret scan.

### 4. No-regression confirmation

**FALS-01 Part B's three sentences — must lint CLEAN (fix works):**

```
R2-4-clean settles_the_unit: Ok(...)      ("The runtime settles the unit once every blocking residual is resolved and evidence is verified.")
R2-4-clean authorizes_budget: Ok(...)     ("When the recovery reserve is untouched, the runtime authorizes additional budget for the next round automatically.")
R2-4-clean grants_authority: Ok(...)      ("During dispatch the runtime grants the unit authority consistent with its declared blast radius.")
```

All three now lint clean — **COR3 fix confirmed working**, matching its
required regression tests (which are also present and passing in
`skills/mod.rs`'s own `#[cfg(test)]` block:
`a_runtime_descriptive_sentence_settling_the_unit_is_not_over_rejected` etc.).

**Critic's original three RES-01 sentences (no "the runtime") — must still
correctly REJECT (fix didn't overcorrect into uselessness):**

```
R2-4-reject settle_itself: Err(ControlLogicDetected { pattern: "settles the unit", line: 5 })
R2-4-reject authorize_budget: Err(ControlLogicDetected { pattern: "authorize additional budget", line: 5 })
R2-4-reject elevate_authority: Err(ControlLogicDetected { pattern: "elevate the unit's authority", line: 5 })
```

All three still correctly reject. **No regression found** — the
runtime-exemption is genuinely selective, not a general widening.

### 5. `REPO_OWNED_SKILLS` scope regression check

Built a throwaway skills-root under the session scratchpad containing all
six real skills plus a seventh `zzz-attacker-skill/SKILL.md` (containing an
overt control-logic violation, "This skill settles the Unit once its
evidence passes."), and ran `lint_installed_skills` against it:

```
R2-5 result count: 6 (expected 6)
R2-5 mentions attacker dir in results: false
R2-5 wayfinder-gauntlet: true
R2-5 grilling: true
R2-5 domain-modeling: true
R2-5 research: true
R2-5 prototype: true
R2-5 gauntlet-run: true
```

**Attack executed and repelled** — the explicit closed-list scope (no
directory glob) holds; the attacker directory never enters the result set;
all six real shipped skills lint clean (`true` = `Ok`), confirming the
currently-shipped `SKILL.md` content still passes under the corrected
mechanism. No new front-matter-bypass attacks attempted per the task
brief's "spend less time here" guidance — round 1's exhaustive front-matter
attack surface (BOM, duplicate key, homoglyph, case mismatch, unterminated
fence) exercises code paths (`gauntlet_md::parse`, `validate_schema`)
untouched by COR3/COR4, so there is no reason to expect a different result;
confirmed via code reading that neither correction touched
`validate_schema`, `normalize_front_matter_keys`, or `lint_installed_skills`.

## Cleanup verification

```
$ git status --porcelain crates/ tests/ .agents/skills/
A  .agents/skills/domain-modeling/PROVENANCE.md
A  .agents/skills/domain-modeling/SKILL.md
A  .agents/skills/gauntlet-run/SKILL.md
A  .agents/skills/grilling/PROVENANCE.md
A  .agents/skills/grilling/SKILL.md
A  .agents/skills/prototype/PROVENANCE.md
A  .agents/skills/prototype/SKILL.md
A  .agents/skills/research/PROVENANCE.md
A  .agents/skills/research/SKILL.md
M  crates/gauntlet-app/src/lib.rs
A  crates/gauntlet-app/src/skills/mod.rs
M  crates/gauntlet-app/tests/skills.rs
M  crates/gauntlet-arch-tests/src/single_writer.rs
A  tests/fixtures/skills/control-logic-violation.md
```

Matches the pre-existing task19 staged changeset exactly (same 14 files,
same status letters as the `git diff --cached --stat` baseline recorded at
the start of this round). The throwaway integration-test file
(`crates/gauntlet-app/tests/zz_falsifier2_attack.rs`) and the throwaway
scratch skills-root (under the session scratchpad, outside the repo) were
both removed before this check. `.wayfinder/answer-key.md` and
`.gauntlet/reference.lock` were not touched.

## Verdict rationale

**CONFIRM.** Every attack surface the task brief specified was actually
executed against the live COR3/COR4-corrected mechanism, not just read and
reasoned about:

- The COR3 runtime-exemption's worst-case gaming (negation/contrast
  clauses) does exist, but reproduces exactly the named, accepted,
  explicitly-out-of-scope example from the correction contract itself
  ("This skill, not the runtime, settles the unit") — not a new residual.
  The same-line-only boundary (the part of the mechanism actually meant to
  hold) held under direct attack.
- FALS-01 Part A's disclosed residual is still real and still honestly
  disclosed under two independently-invented new phrasings — not silently
  claimed fixed, not newly broken.
- FALS-02's fix holds under the original four reproductions plus five new
  boundary/convention variants; the two "false-positive risk" checks that
  do reject are correct per the project's own narrowly-scoped `$VAR`-only
  indirection convention, not bugs.
- No regression: COR3's three required-clean sentences lint clean; the
  critic's three original violation sentences still correctly reject.
- `REPO_OWNED_SKILLS` scope holds against a live attacker-directory
  injection; all six real shipped skills still lint clean.

0 blocking, 0 major, 0 minor. No correction round required. Task 19's skill
lint mechanism is settled from this falsifier's perspective.
