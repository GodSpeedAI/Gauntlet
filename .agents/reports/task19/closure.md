# Task 19 closure — companion + repo-owned skills

**Settled:** 2026-08-31, orchestrator (this session).
**Plan reference:** `.agents/plans/gauntlet-impl.md` Task 19. Deps: Task 18
(settled 827dd46). Parallel-safe per the plan.

## What landed

Two parallel builder slices (design contract:
`.agents/reports/task19/IN-FLIGHT.md`):

- **Slice A (content):** four companion skills ported from real upstream
  (`github.com/mattpocock/skills` @ `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`,
  same commit as task18, MIT) — `grilling`, `domain-modeling`, `research`,
  `prototype` — each under `.agents/skills/<name>/{SKILL.md,PROVENANCE.md}`
  with honest delta summaries. Plus `.agents/skills/gauntlet-run/SKILL.md`,
  repo-owned (no PROVENANCE.md), the operator procedure
  `doctor → reference → run → supervise escalations → inspect evidence`,
  correctly refusing to claim `gauntlet run` (Task 20) already works.
  Per-role `gauntlet-role-<role>` skills deliberately NOT authored — all
  ten `.agents/roles/*.md` files audited and found self-contained
  (42–55 lines), decision recorded in IN-FLIGHT.md rather than silently
  skipped.
- **Slice B (code):** `gauntlet_app::skills` — a skill lint (front-matter
  schema validation, a control-logic denylist, a secret-like-value scanner,
  an absolute-path scanner) over an explicit closed `REPO_OWNED_SKILLS`
  list (never a directory glob — untracked local tool skills like `jolli*`
  under `.agents/skills/` are never scanned or asserted about). Tests
  added to the existing `crates/gauntlet-app/tests/skills.rs` (task18's
  binary), gate `cargo nextest run -p gauntlet-app skills::lint`.

## Confirmation history

1. Fresh blind design-as-built critic: **CONFIRM** (0 blocking / 1 major /
   1 minor). RES-01 (major): the control-logic denylist's recall was
   narrow — three plausible rephrasings of a settlement/authority/budget
   decision evaded it. RES-02 (minor): no explicit "what this lint is not"
   disclosure comparable to `single_writer.rs`'s THREAT MODEL precedent.
   Also independently confirmed the `single_writer.rs` boundary-file edit
   (adding `skills/mod.rs` to `ANSWER_KEY_SURFACE_FILES`, never to
   `WRITE_API_INVENTORY`) is a genuine documented lexical false positive,
   not a weakened boundary.
2. **COR1** (widened `CONTROL_LOGIC_PHRASES` with 7 targeted additions
   closing the critic's three exact sentences) **+ COR2** (added the
   disclosure section) — landed, all gates green.
3. Fresh blind EXECUTING falsifier round 1: **NOT_CONFIRM** (1 major /
   1 minor). FALS-01 (major): COR1's own widening was subject-blind and
   now over-rejected legitimate, sanctioned third-person descriptive prose
   about what *the runtime* does (e.g. "the runtime settles the unit
   when..." — exactly `gauntlet-run`'s own required voice) — a structural
   defect in the mechanism, not a one-off string. FALS-02 (minor): a
   compound secret-label convention (`api_key:`, `secret_key:`, etc.)
   bypassed the scanner via an underscore word-boundary bug. Every other
   attack surface (REPO_OWNED_SKILLS scope, front-matter bypasses —
   BOM/duplicate-key/homoglyph/case/unterminated-fence — CRLF and
   code-fence robustness, and a LIVE re-attack on the `single_writer.rs`
   boundary edit by actually planting an offending file and running `just
   boundaries`) held under direct execution.
4. **COR3** (a "the runtime" same-line proximity exemption for
   `CONTROL_LOGIC_PHRASES` matches only — never for
   `CONTROL_LOGIC_IDENTIFIERS`, which stay unconditional — fixing the
   subject-blindness structurally rather than patching more phrases) **+
   COR4** (fixed the secret-label boundary check: only an alphanumeric
   character before a label counts as a boundary violation, not `_`)
   — landed with 9 new regression tests, all gates green.
5. Fresh blind EXECUTING falsifier round 2 (different agent instance):
   **CONFIRM** (0 blocking / 0 major / 0 minor). Directly attacked the
   COR3 exemption's own boundary (negation/contrast-clause gaming, a
   cross-line split attempt); the gaming case reproduces exactly the
   correction contract's own named, accepted-out-of-scope example, not a
   new gap, and the same-line-only boundary held. Re-confirmed FALS-01
   Part A's disclosed residual is still real and honestly disclosed (not
   silently claimed fixed) under two newly-invented phrasings. Re-verified
   FALS-02's fix against the original four reproductions plus five new
   boundary/convention variants — all caught. Confirmed no regression:
   COR3's required-clean sentences lint clean, the original violation
   sentences still reject. Re-confirmed `REPO_OWNED_SKILLS` scope against
   a live attacker-directory injection.

Orchestrator personally ran the full gate battery after every round
(`just check`, `just test`, `just lint`, `just deny`, `just boundaries`,
`just ruler`, `just doctor`, plus the exact literal gate) — never settled
on builder/subagent say-so. Final battery: workspace 1281/1281, boundaries
228/228, lint clean, deny ok (pre-existing Zlib-allowance/winnow-duplicate
advisories, unrelated to this task, no new dependency added), ruler PASS,
doctor exit OK.

## Residuals (trigger-recorded, not blocking)

- **Control-logic denylist recall (RES-01/FALS-01 Part A):** still a
  disclosed, accepted residual — a lexical exact-phrase/identifier scanner
  cannot achieve semantic completeness over open-ended English paraphrase.
  Documented explicitly in `skills/mod.rs`'s "## What this lint is not"
  section. Trigger: next time a real skill author reports a false negative
  in practice, or the next `skills/mod.rs` touch.
- **COR3 exemption gaming via negation/contrast clause** (falsifier round
  2, item 1a): "Unlike the runtime, this skill authorizes budget directly"
  exempts, same as the correction contract's own named out-of-scope
  example. Same residual class as above, not a new one.
- **Bare Windows path with forward slashes** (falsifier round 1, item 4):
  `C:/Users/...` (no backslash) isn't caught by `scan_absolute_paths`; low
  practical risk on this repo's Linux-only toolchain, noted not filed.

## Files changed (14, all under this task's declared scope)

```
.agents/skills/{grilling,domain-modeling,research,prototype}/{SKILL.md,PROVENANCE.md}
.agents/skills/gauntlet-run/SKILL.md
crates/gauntlet-app/src/skills/mod.rs (new)
crates/gauntlet-app/src/lib.rs (+1 line: pub mod skills;)
crates/gauntlet-app/tests/skills.rs (+5 tests)
crates/gauntlet-arch-tests/src/single_writer.rs (ANSWER_KEY_SURFACE_FILES +1 entry, documented lexical false positive)
tests/fixtures/skills/control-logic-violation.md (new)
```

No file outside this footprint was touched. `.wayfinder/answer-key.md` and
`.gauntlet/reference.lock` untouched at every checkpoint (verified via
`just ruler` after every round).
