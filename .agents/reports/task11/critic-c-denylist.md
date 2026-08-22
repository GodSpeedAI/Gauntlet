# Adversarial review: Task-11 widening of SPEC_VOCABULARY_SEGMENTS (Critic C)

> Persisted by the orchestrator from the critic's final message (read-only
> mandate). Content verbatim. Subject: `["model"]` -> `["model","digest","check"]`
> in `crates/gauntlet-arch-tests/src/denylist.rs`.

**Verdict: NO CRITICAL, NO MAJOR. The change does not unmask anything that previously denied. Two MINOR documentation defects (an unsupported "~75" figure and one overclaiming comment) and one recorded-residual observation. The claimed spec-vocabulary justification is genuine per the in-repo spec, and the pin tests have real teeth in both directions.**

Evidence sources: static reading of denylist.rs, lib.rs, walk.rs, git history/diff, literal censuses via `git grep` at HEAD, and Core-spec.md. No compilation, no test runs.

## Q1. Masking — matching is segment-exact; no previously-denied spelling now passes

- `SPEC_VOCABULARY_SEGMENTS` is consulted in exactly one place: `denylist.rs:258-262`, gating whether an adapter's LAST name segment enters `adapter_segments`. That list is consumed via `segment_literal` (`denylist.rs:320-326`): `trim().to_lowercase()` then **exact `==` equality** — never substring. Fires only for `Role::Inner` files (domain/ports/app).
- **Nothing was unmasked, because the segments never existed before.** At HEAD the workspace had no adapter ending in `digest` or `check`; the diff touches only the const, a comment, the derive gate, and tests — `DENIED_TERMS` and the substring spelling derivation are byte-identical. No spelling that denied at HEAD passes now.
- **Vendor spellings were never in this rule's blast radius.** `rust-digest`, `checkstyle`, `preflight-check`, `digest-auth` never matched (exact-whole-literal matching) before or after; the substring matcher only sees `DENIED_TERMS` (unchanged, contains none of these words). A domain `Cargo.toml` key `checkstyle = "10"` passes today exactly as at HEAD — a pre-existing vendor-list coverage question, not a Task-11 regression.
- **The real (accepted) residual:** in an inner crate, `let rung = "check";` or `match kind { _ => "digest" }` can never be an AK-014 finding, even for future adapters ending in those segments, whereas the analogous `"diff"` denies (`denylist.rs:1027-1037` pins the contrast). Same deliberate non-distinguishing ruling as `model`; bounded by adapter selection resolving from configuration files outside inner crates.

## Q2. Collision reality — genuinely spec vocabulary

The rule-as-written collision set (exact whole literals, case-insensitive, trim-padded) in inner crates at HEAD is **15 occurrences, all verified**: `expect("digest")` test labels ×6 (compare/recording.rs:353, compare/review.rs:741,915, comparator.rs:1706,1709,1718); doctor clause name "digest" ×4 (doctor/checks.rs:446, doctor/mod.rs:865,908,954); reference-manifest key "digest" ×3 (reference/manifest.rs:546,613,617); the domain type's own name `"Digest"` (values.rs:73); `"check"` as a synthetic command name (observer_evidence.rs:626). None is a vendor identifier. Spec citations verified: §5.2 `Digest`, §14.2 rung ladder, §9.4 `reference_digest`, §12.4 `observation_digest`, §13.2/I4 `context_envelope_digest`. The §8.5 "doctor-check vocabulary" claim is only generically supported.

**MINOR — the "~75 pre-existing literals" figure is unsupported.** No constructible set equals 75 (exact-literal collisions in policed scope = 15; all-core = 34; whole-repo = 38-39 (54 with model); containing-the-words in inner crates = 277; word occurrences inside string literals = 137). The DIRECTION is verified true; the number is not. Absence of a 75-matching set holds only within these measured scopes.

## Q3. Pin strength — genuine teeth in all four directions

`spec_vocabulary_segments_are_pinned_and_the_spellings_still_deny` (`denylist.rs:1048-1117`): (a) 15 spellings including `verify-file-digest`, `digest-file-verify`, `verify-schema-check`, `check-schema-verify` scanned `Role::Inner` against a real member list — each must violate, via the production substring mechanism; (b) bare words `model`/`digest`/`check` assert zero violations. Re-narrow fails twice (content-equality pin + bare-word loop); re-widen fails the content-equality pin; deleting the exclusion clause fails the `!adapter_segments.iter().any(...)` assert + bare-word loop. No pin asserts each entry is the last segment of a REAL adapter (provenance unpoliced) — same class as the `DENIED_TERMS` pin, MINOR-at-most.

Cross-check: `the_maskable_wire_key_intersection_is_pinned` recomputes from the real member list; `ANNEX_B_WIRE_KEYS` contains no `digest`/`check` key; the newly derived-but-not-excluded segments `code` (`verify-command-exit-code`) and `report` (`verify-test-report`) are not wire keys either — the intersection stays `["command"]`; the exclusion shrinks nothing the Annex-B mask could hide.

## Q4. Precedent consistency — same adjudicated class; one comment overclaims

Task 10 landed `["model"]` in commit `1dab05c` with the identical shape (const + rationale doc + derive gate + pin test + maskable-pin note) and a recorded two-option adjudication. Task 11 follows the chosen class; the wire-key-masking option was structurally unavailable here too (of the 15 colliding literals only observer_evidence.rs:626 is in a parser module, and it is a function-argument position, not key-access).

**MINOR — in-code doc overstates coverage.** The comment claims the spellings `digest-file` and `check-schema` "ARE still derived and denied" (`denylist.rs:105-107`). The derive joins ALL reversed segments (`denylist.rs:236-241`), producing `digest-file-verify` / `check-schema-verify`; a literal exactly `"digest-file"` or `"check-schema"` contains no derived term and PASSES. The pin test uses the correct full spellings, so behavior is right and the comment is wrong (harmless direction, but misdescribes the denied set to future editors).

## Findings summary

| # | Severity | Finding | Evidence |
|---|---|---|---|
| 1 | MINOR | "~75 pre-existing literals" figure matches no measurable set; true rule-relevant collision set is 15 | builder-checkpoints.md:207 vs census |
| 2 | MINOR | Comment names `digest-file`/`check-schema` as derived-and-denied; actual derived spellings are `digest-file-verify`/`check-schema-verify`; bare `"digest-file"` passes | denylist.rs:105-107 vs 236-241; pin correct at 1085,1090 |
| 3 | Observation (accepted residual) | Bare `digest`/`check` literals in inner layers can never be AK-014 findings — the `model`-class residual, deliberately pinned | denylist.rs:93,258-262,1108-1116 |
| 4 | Observation | Sibling adapters' segments `code`/`report` derived but not excluded; zero pre-existing exact literals collide — the line drawn is consistent | working-tree Cargo.toml members; HEAD censuses |

**Scope declaration (absence claims hold only within):** denylist.rs/lib.rs/walk.rs read; git diff HEAD + commit 1dab05c for denylist.rs; HEAD-vs-tree Cargo.toml member lists; git grep literal censuses at HEAD over inner crates, all core crates, and whole repo; spec headers and vocabulary lines; the two builder-checkpoint reports. Not searched: escapes.rs/literals.rs diffs (grep confirmed neither consumes `adapter_segments`), remaining task11 critic reports, anything outside this repo.
