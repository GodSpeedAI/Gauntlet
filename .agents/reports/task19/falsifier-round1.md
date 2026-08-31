# Falsifier round 1 — Task 19 (companion + repo-owned skills)

**Verdict: NOT_CONFIRM** (1 major, 1 minor found; 0 blocking)

Fresh-context, blind, EXECUTING adversarial round against the corrected tree
(post-COR1/COR2). Did not read any `*-progress.md` or critic-design.md's
"Suggested correction" prose. Independently re-attacked; did not check the
critic's homework. Every finding below was reproduced by actually running
code (temporary integration-test files added to
`crates/gauntlet-app/tests/`, one temporary file added to
`crates/gauntlet-cli/src/`, then removed — tree verified clean via
`git status`/`git diff --cached --stat` at the end, matching the pre-existing
task19 changeset exactly).

## Gate reproduction (run independently)

```
$ cargo nextest run -p gauntlet-app skills::lint
Summary [0.010s] 5 tests run: 5 passed, 428 skipped

$ just boundaries
Summary [18.025s] 228 tests run: 228 passed, 0 skipped
(includes answer_key_single_writer, PASS)

$ just ruler
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock

$ just test-package gauntlet-app
Summary [0.566s] 433 tests run: 433 passed, 0 skipped
```

All four numbers match critic-design.md's independently-run numbers
(5/5, 228/228). No discrepancy found in gate reproducibility.

## Residuals

### FALS-01 — control-logic denylist: new under-rejections AND a confirmed over-rejection from COR1 itself
- **Dimension:** architecture / teeth
- **Severity:** major
- **Locus:** `crates/gauntlet-app/src/skills/mod.rs:253-266` (`CONTROL_LOGIC_PHRASES`)
- **Contract ref:** plan Task 19 Gate ("a skill containing a settlement decision fails it (teeth)"); Core-spec Annex D.4 ("anything that decides settlement, authority, or budget belongs in the runtime"); IN-FLIGHT.md Slice B ("Do not denylist a bare word... If real installed-skill text trips a false positive, fix the phrasing... never widen the lint into uselessness").

**Part A — recall still narrow (new sentences, different phrasing from COR1's additions and the critic's original three).** Executed via `lint_skill_text` against a throwaway `crates/gauntlet-app/tests/zz_falsifier_attack.rs` (removed after the run; `cargo test -p gauntlet-app --test zz_falsifier_attack -- --nocapture`):

```
lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
    The skill should close this Unit once review finishes.\n")
// -> Ok(...)  (a settlement decision — "close this Unit" — passes clean)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
    Bump the budget ceiling up before continuing the run.\n")
// -> Ok(...)  (a budget-increase instruction passes clean)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
    Treat this agent as having lead_agent authority for the rest of the run.\n")
// -> Ok(...)  (an authority-elevation instruction, different verb form than
//              COR1's "elevate...authority"/"grants the unit authority",
//              passes clean — exactly the AGENTS.md-guarded inverse case)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
    If tests are green, the Outcome is Pass for this unit.\n")
// -> Ok(...)  (a paraphrase of Outcome::Pass with no "::" passes clean)
```

Observed output (verbatim from the run):
```
A1a close_this_unit: OK (passed clean)
A1b bump_budget: OK (passed clean)
A1c treat_as_authority: OK (passed clean)
A1d outcome_is_pass: OK (passed clean)
```

This is the same class of gap RES-01 already disclosed as accepted residual
(the module doc's "What this lint is not" section says exactly this: "A
settlement, authority, or budget decision phrased in yet another way...
can still evade it entirely — this is a disclosed, accepted residual, not
an oversight"). I am **not** re-filing RES-01 itself; I list Part A only to
establish, with fresh executed evidence, that the disclosed residual is
real and immediately reproducible with trivial rephrasing effort (each
sentence took under a minute to construct, no adversarial cleverness
required) — consistent with the disclosure's own honesty, not contradicting
it.

**Part B — the more serious finding: COR1's own widening now over-rejects legitimate, exactly-precedented descriptive prose.** The module's own passing precedent (test `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`, `skills/mod.rs:700-710`) establishes that third-person descriptive prose about what the *runtime* does — "the runtime settles **a** Unit when its evidence is verified, under its own authority model" — must lint clean. COR1 added the phrase `"settles the unit"` (definite article) to catch a skill *claiming to settle the Unit itself* (critic-design.md RES-01's reproduced sentence: "This skill settles the Unit once its evidence passes"). But the phrase is a bare substring match with no subject-tracking, so it ALSO fires on the exact same descriptive-prose pattern the design explicitly sanctions, merely because the article changed from indefinite to definite:

```
lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
    The runtime settles the unit once every blocking residual is resolved \
    and evidence is verified.\n")
// -> Err(ControlLogicDetected { pattern: "settles the unit", line: 5 })
```

Reproduced via `crates/gauntlet-app/tests/zz_falsifier_attack2.rs` (removed
after the run); observed output verbatim:
```
B1a runtime_settles_the_unit_descriptive: REJECTED -> ControlLogicDetected { pattern: "settles the unit", line: 5 }
```

The same failure mode reproduces for two more of COR1's seven additions —
`"authorizes additional budget"` and `"grants the unit authority"` — against
equally legitimate third-person descriptive sentences about the runtime:

```
"When the recovery reserve is untouched, the runtime authorizes additional \
 budget for the next round automatically."
-> Err(ControlLogicDetected { pattern: "authorizes additional budget", line: 5 })

"During dispatch the runtime grants the unit authority consistent with its \
 declared blast radius."
-> Err(ControlLogicDetected { pattern: "grants the unit authority", line: 5 })
```

None of the five shipped `SKILL.md` files happen to contain these exact
strings today, so the gate stays green — this is a latent authoring trap,
not a shipped defect. But it directly contradicts the design's own stated
avoidance principle ("Do not denylist a bare word... that will
false-positive on this very skill's own procedural prose") in spirit: a
lexical substring match with no subject-tracking cannot distinguish "the
skill/agent decides X" (forbidden) from "the runtime does X" (the
`gauntlet-run` skill's entire required voice, per IN-FLIGHT.md: "every such
decision is a sentence like 'the runtime settles a Unit when X'... never
'settle the unit when X'"). COR1 closed three under-rejections by adding
phrases general enough to also catch the *sanctioned* voice whenever the
article or plurality differs from the specific fixture sentence it was
tuned against. A future skill author extending `gauntlet-run`'s own
documentation in its own established voice has a nontrivial chance of
tripping this lint on correct, desired content — which is the "lint that's
unusable" failure mode the task brief flagged as also worth finding,
distinct from under-rejection.

- **Reproduction commands:** the two throwaway test files above, run with
  `cargo test -p gauntlet-app --test zz_falsifier_attack -- --nocapture` and
  `cargo test -p gauntlet-app --test zz_falsifier_attack2 -- --nocapture`.
  Both files were deleted after observation; `git status --porcelain
  crates/` confirms nothing stray remains.
- **Not applied (falsifiers report, never repair).**

### FALS-02 — secret-like scan: compound label word-boundary bypass
- **Dimension:** evidence / security
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/skills/mod.rs:569-587` (`label_assignment_is_secret`)
- **Contract ref:** plan Task 19 Steps ("no secrets"); spec §8.1 ("Secrets by indirection only"); IN-FLIGHT.md Slice B ("a literal `key:`/`token:`/`password:`/`secret:` assignment... whose value does not start with `$`").

**Defect:** `label_assignment_is_secret`'s left-boundary check treats an
underscore as a boundary-violating character (same as an alphanumeric),
which is correct for avoiding `"monkey:"` matching label `"key"` but also
means a *compound* label using an underscore — `api_key:`, `secret_key:`,
`auth_token:`, `client_secret:` — never satisfies the boundary check for
any of the four `SECRET_LABELS` (`key`, `token`, `password`, `secret`),
because the character immediately before the label substring is always
`_`. None of `SECRET_PREFIXES` fire either when the value itself is a
generic-looking credential string with no recognized vendor prefix. The
combination is a full bypass for a very common secret-labeling convention.

**Reproduction** (`crates/gauntlet-app/tests/zz_falsifier_attack2.rs`,
`attack_secret_label_compound_bypass`, removed after the run):

```
lint_skill_text("x", "---\nname: x\ndescription: d\n---\napi_key: q7f2mZpLX9rT4vN8cKdE3wYh\n")
-> Ok(...)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\nsecret_key: q7f2mZpLX9rT4vN8cKdE3wYh\n")
-> Ok(...)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\nauth_token: q7f2mZpLX9rT4vN8cKdE3wYh\n")
-> Ok(...)

lint_skill_text("x", "---\nname: x\ndescription: d\n---\nclient_secret: q7f2mZpLX9rT4vN8cKdE3wYh\n")
-> Ok(...)
```

Observed output verbatim:
```
B2a api_key_label: OK (passed clean)
B2b secret_key_label: OK (passed clean)
B2c auth_token_label: OK (passed clean)
B2d client_secret_label: OK (passed clean)
B2e control_bare_secret_label: REJECTED -> SecretLikeValueDetected { line: 5 }
```

(`B2e`, the bare `secret:` label with the same value, correctly rejects —
confirming the gap is specifically the compound-label boundary, not the
value shape.) Severity kept at minor rather than major because: none of the
five shipped `SKILL.md` files use a compound label (confirmed by reading
all five in full — none contains any `key:`/`token:`/`password:`/`secret:`-
shaped line at all except the deliberately-indirected examples in the
module's own doctests), and `SECRET_PREFIXES` still catches any value using
a recognized vendor token shape (`sk-`, `ghp_`, `AKIA`, `xox[bpa]-`)
regardless of label. The gap is real but narrower than a full secret-scan
defeat.

- **Not applied (falsifiers report, never repair).**

## Attacks executed and repelled (defenses held — recorded per task brief's "distinguish attack executed and repelled from... found a gap")

1. **REPO_OWNED_SKILLS scope boundary.** Executed `lint_installed_skills`
   against: (a) the real `.agents/skills/` root with an extra adversarial
   7th directory `zzz-attacker-skill/SKILL.md` (containing an obvious
   control-logic violation) added alongside the six real ones — result
   count stayed exactly 6, the attacker directory never appeared in the
   results (`mentions attacker dir in results: false`); (b) a scratch root
   with 5 of the 6 real names present and `prototype/` entirely absent —
   result count stayed exactly 6, and the `prototype` slot reported
   `Err(Io { path: ".../prototype/SKILL.md", message: "No such file or
   directory (os error 2)" })` rather than a silent pass or being dropped
   from the vector — `lint_passes_every_installed_skill`'s `assert_eq!` on
   `.len()` plus the `Ok`-only filter would correctly fail this case.
   **No misleading-pass scenario found.** (`attack_repo_owned_scope`,
   removed after the run.)

2. **Front-matter schema bypass.** Executed against: UTF-8 BOM prefix
   (rejected — `FrontMatterGrammar { source: ByteOrderMark }`), duplicate
   `name` key with different values (rejected —
   `FrontMatterGrammar { source: DuplicateKey }`), Cyrillic homoglyph in
   `name` (`х` U+0445 vs ASCII `x` — rejected as `NameMismatch`, exact
   string comparison is not confused by visual similarity), directory-name
   case mismatch (`X` vs `x` — rejected as `NameMismatch`, no
   case-insensitive bypass), unterminated fence (rejected —
   `UnterminatedFrontMatter`). CRLF line endings and a fence line with
   trailing whitespace (`--- `) both parse successfully via the shared
   `trim()`-based fence detection — this is lenient but not a security
   bypass (verified separately that CRLF documents still trigger
   control-logic/secret/absolute-path detection correctly — see below).
   **No schema-bypass gap found**; the two lenient-parse cases are noted as
   informational, not residuals.

3. **CRLF robustness of the body scans.** A CRLF-terminated document
   containing `SettlementService::settle_unit`, a `sk-` secret, and an
   absolute path all triggered their correct, respective errors at the
   correct line numbers (`ControlLogicDetected`/`SecretLikeValueDetected`/
   `AbsolutePathDetected`, all at line 5). **No CRLF-based evasion found.**

4. **Absolute-path scan.** Windows-style backslash path
   (`C:\Users\name\secret.txt`) correctly rejected as
   `AbsolutePathDetected`; a Unix absolute path and a secret literal each
   embedded inside a fenced code block (` ```...``` `) were still caught
   (the scan is whole-document, not prose-only, which the module docs
   state explicitly — confirmed behaviorally). A Windows path written with
   forward slashes (`C:/Users/name/secret.txt`, no backslash) was **not**
   caught (`windows_drive_path` only recognizes the `X:\` spelling and the
   generic `/`-prefix check does not match a bare drive letter) — noted as
   a known, narrow gap but not filed as a separate residual: the design
   comment already scopes the Windows check to the `\` spelling
   specifically, and a forward-slash Windows path is not a path any tool on
   this repo's own Linux-only toolchain would emit, so its practical attack
   surface for this repo is low; recorded here for completeness rather than
   as a residual requiring correction.

5. **`single_writer.rs` boundary — actually broken and observed to fail.**
   Added a throwaway file `crates/gauntlet-cli/src/zz_falsifier_probe.rs`
   (a core crate, not `gauntlet-app`) containing the literal string
   `"wayfinder-gauntlet"`, then ran `just boundaries`. Observed:
   ```
   FAIL [ 12.681s] (225/228) gauntlet-arch-tests::architecture_boundaries answer_key_single_writer
   single-writer: 1 violation(s)
     - single-writer: 'crates/gauntlet-cli/src/zz_falsifier_probe.rs' names the
       answer-key surface in production code (["wayfinder"]) but is not in the
       pinned inventory — ... extend ANSWER_KEY_SURFACE_FILES with a reason or
       remove the mention
   ```
   The boundary suite caught it immediately and correctly (Rule A fires on
   ANY core-crate file naming the token, not just `gauntlet-app`). File
   removed; reran `just boundaries` → 228/228 passed clean. **Attack
   executed and repelled** — this is a real, demonstrated defense, not an
   assumption from reading the code.

## Scope note

Per the task brief, I did not read any `.agents/reports/task19/*-progress.md`
file or critic-design.md's "Suggested correction" prose. I did read
critic-design.md's Summary/Residuals/reproduction sections for context, per
instruction. FALS-01 Part A deliberately does not re-litigate RES-01
(already disclosed, already accepted as residual) — it is filed together
with Part B (the over-rejection direction, which is new) under one id
because both stem from the same root cause: `CONTROL_LOGIC_PHRASES` is a
subject-blind substring match, so any widening that closes an
under-rejection by adding a general-enough phrase risks opening an
over-rejection on the sanctioned descriptive voice, and vice versa. That
tension is structural to the lexical-substring approach chosen, not a
one-off oversight — which is exactly what a **major** (not blocking)
severity is meant to convey: real, reproducible, worth a correction round,
but not a reason to block settlement of Task 19's stated Done-when (one
control-logic fixture must fail — it does; none of the five shipped skills
trip the over-rejection case — confirmed by reading all five in full).

## Verdict rationale

**NOT_CONFIRM.** FALS-01 is major (a confirmed, executed defect in both
directions of the same mechanism — new under-rejections beyond what RES-01
already disclosed is not new, but the COR1-introduced over-rejection on
sanctioned descriptive prose is a new, previously untested failure mode
that a correction round should address, e.g. by anchoring the "settles"/
"authorizes"/"grants" phrases on a first- or second-person or imperative
subject pattern rather than bare substring, or by requiring the
runtime-noun `AGENTS.md` already uses ("the runtime") to be ABSENT nearby
before firing). FALS-02 is minor (a real but narrow secret-label bypass,
no shipped skill affected). Both are correctable within the existing
design's stated constraints without widening the lint into uselessness or
narrowing it into vacuity. No blocking finding: `REPO_OWNED_SKILLS` scope,
front-matter schema, CRLF/code-fence robustness, and the `single_writer.rs`
boundary edit all held under direct adversarial execution.
