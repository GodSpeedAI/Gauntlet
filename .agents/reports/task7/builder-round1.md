# Task 7 — Builder round 1 report: ReferenceProvider (normalize, digest, freeze, verify)

**Builder:** FINISHER dispatch 2 (write-only session, 2026-08-18), completing a
round opened by earlier sessions (D1–D3 landed 2026-08-17; D4 landed
2026-08-17, verified COMPLETE-WITH-GAPS by the read-only reviewer in
`.agents/reports/task7/d4-verify.md` and remediated here as D4R).
**Verification-method legend** used throughout: `[ran]` = the named gate was
actually executed and observed by a builder session (earlier sessions only;
every one of those runs predates this finisher session); `[read]` = verified
by reading the code/artifact only, in this session, with no execution;
 `[none]` = not verified at all here. **This finisher session ran NO cargo,
rustc, just, or test command** (ESC-002: the host OOM-kills compiling
sessions). Every gate listed below is for the orchestrator to run.

## Artifact

| File | Lines | Status |
|---|---|---|
| `crates/gauntlet-app/src/reference/mod.rs` | 18 | D1, `[read]` |
| `crates/gauntlet-app/src/reference/normalize.rs` | 286 | D1, `[read]` in full |
| `crates/gauntlet-app/src/reference/parse.rs` | 930 | D2, `[read]` in full |
| `crates/gauntlet-app/src/reference/manifest.rs` | 1270 | D3, `[read]` header + error enum + `build_definition` region only |
| `crates/gauntlet-app/src/reference/provider.rs` | 639 | D4 (earlier session) + D4R (this session), `[read]` in full |
| `crates/gauntlet-app/src/doctor/checks.rs` | 1108 | D7 (this session) — check 2 rewritten, strict lock reader added |
| `crates/gauntlet-app/src/doctor/mod.rs` | 1075 | D7 (this session) — Scratch fixture + lock helper reworked, 3 new tests |
| `crates/gauntlet-cli/src/main.rs` | 151 | D5 (this session) — `reference digest` subcommand |
| `crates/gauntlet-cli/Cargo.toml` | 35 | D5 (this session) — `gauntlet-adapter-evidence-fs` dev-dep |
| `crates/gauntlet-cli/tests/p1_reference_integrity.rs` | 369 | D6 (this session), new |
| `.agents/reports/reference-candidates.md` | 135 | D8 (this session) — two dated entries appended |

Untouched by design (frozen ruler): `.wayfinder/**`,
`.gauntlet/reference.lock`, `scripts/reference-digest.sh`; also untouched:
`deny.toml`, the arch suite, `.agents/CURRENT_STATUS.md`, the spec, the plan.

## Decision log

1. **Normalization semantics (D1, earlier session; `[read]`-verified).**
   Byte-exact reproduction of `scripts/reference-digest.sh`: CRLF/CR→LF
   folding, split-on-`\n` + pop trailing `""` elements + rejoin with exactly
   one trailing LF (empty → zero bytes), no trimming, no sorting. Pinned by
   exact-output synthetic tests, not only digests.
2. **Per-block parse isolation (D2, earlier session; `[read]`-verified).**
   `parse_answer_key` returns well-formed Units AND per-block failures with
   loci (plan Task 7 step 4); `parse_all_or_fail` is the all-or-nothing
   wrapper. Strict `wire_enum!` validation everywhere (AK-004).
3. **Manifest deferrals (D3, earlier session; `[read]`-verified in the
   cited regions).** Deliberate, documented, not invented:
   `quality_dimensions` parsed for shape but NOT materialized into the
   domain `Reference` (their `evidence_required` are bare locator strings;
   the domain requires kind+locator+producer — inventing mappings would
   invent the ruler's semantics; waits for the comparison-contract task);
   `settlement.required_evidence` carried verbatim as prose locators,
   constructed `SettlementCriteria.required_evidence` empty, same reason;
   `minimum_reliability_passes` extracted from the manifest's own
   ">= N settlements" prose (present and numeric or a typed error);
   `terminal_conditions` through `TerminalState::parse`; `success_criteria`
   verbatim strings; `hard_units` empty (the manifest names none).
4. **EvidenceKind::File for the frozen key (D4, earlier session).** Freeze
   stores the normalized answer-key bytes via `put(bytes, EvidenceKind::File,
   …)`; the store refuses same-bytes-different-kind, making the kind choice
   permanent — a frozen ruler artifact is a file-shaped evidence object.
5. **Digest identity semantics (D4, earlier session).** Port `digest` is
   identity, not re-derivation: `normalize` computed the digest over the
   normalized answer-key bytes and carried it into the definition; the
   manifest's own digest stays the lock's business.
6. **Artifact-source seam (D4, earlier session).** `AnswerKeySource`
   (`Path` | `Shared(Rc<RefCell<String>>)` ) and `ManifestSource`
   (`Path` | `Inline`) are the entire seam — the frozen Task-4 contract
   suite drives the artifact as a shared string it rewrites mid-run; nothing
   else branches on provenance.
7. **F1 deviation, now with its compensating controls real (D4 → D7).**
   The reviewer's major finding: per-block parse failures do NOT surface
   through port `normalize` as the original brief sentence said — the frozen
   shared contract suite feeds the plain phrase `"the standard text"` and
   expects `normalize` to succeed, so a strict port `normalize` would fail
   the suite by construction (the brief is self-contradictory there). The
   recorded decision stands: domain invariants in `normalize`; structural
   strictness at admission. What this session added: the module header now
   cites EXACTLY the three strict-admission callsites that exist in the
   final tree (reviewer F2) — `parse_all_or_fail` (`reference/parse.rs`),
   doctor check 2's "unit schema" deep sub-result (`doctor/checks.rs`, made
   real by D7), and the P1 proof's clean-parse assertion
   (`gauntlet-cli/tests/p1_reference_integrity.rs`, D6). No forward
   references remain. The inaccurate header phrase "a malformed block yields
   an empty unit list" was corrected to "yields that block's Unit missing".
8. **F4: closed in code, not recorded as a residual (D4R, this session).**
   `freeze` now digests the bytes it actually reads
   (`digest_normalized(&normalized)`) and, when they differ from the digest
   captured at resolve time, returns the EXISTING
   `ReferenceMutated { expected, actual }` variant — no new error variant, no
   port change — so the stored evidence and the returned version can never
   silently disagree. Judgment: this is ~12 lines inside an existing
   mismatch-shaped branch, not over-engineering; a new test
   (`a_mutation_between_normalize_and_freeze_fires_the_tripwire_at_freeze`)
   exercises it.
9. **F3/F5 remediation (D4R, this session).** New test covers both
   `verify_unchanged` Unavailable arms (artifact removed after freeze;
   artifact made non-UTF-8 after freeze → `Unavailable`, never
   `ReferenceMutated`, with the invalid-UTF-8 class named). Test temp naming
   in `provider.rs` and the doctor `Scratch` fixture converted to the pid +
   atomic-counter idiom.
10. **D5 CLI output contract (this session).** Exactly one line
    `sha256:<lowercase hex>` (the domain `Digest` Display; hex is lowercase
    by construction) via `println!`; missing/unreadable/non-UTF-8 → stderr +
    exit 1; usage errors (unknown arg, missing/doubled `--path`) → exit 2,
    mirroring `doctor`'s manual arg-loop. No new external dependency: the
    command calls `gauntlet_app::reference::normalize::digest_of`.
11. **D7 strict lock reader (this session).** `read_reference_lock` accepts
    exactly the frozen lock's known field set (`LOCK_FIELDS`); unknown fields
    are findings naming the field; `answer_key_digest` must parse as a
    domain `Digest`; `unit_count` must be a non-negative integer; a present
    `algo` must equal the defaults-table `DIGEST_ALGO`. Check 2's "unit
    schema" sub-result parses the configured answer key with the strict
    parser (N/N well-formed, or per-block failure loci as evidence;
    zero-block keys are a failure, not a vacuous pass); the "digest"
    sub-result recomputes the normalized digest and compares to the lock —
    mismatch evidence begins "REFERENCE MOVED" and names it a reference-class
    failure (I1). The lock-finding lands in the "answer key" count clause
    (the count criterion needs the lock) and in the digest clause; unit
    schema still runs whenever the key is readable. The early-return branch
    with `UNIT_SCHEMA_LATER`/`DIGEST_LATER` unavailables is gone — those
    constants are deleted.
12. **Doctor test updates encode now-available truth (this session).** The
    scratch fixture's answer key became a real two-block AK shape and its
    lock a strict freeze record carrying the key's true digest; the healthy
    test now expects check 2 to PASS with all four clauses passing; the
    wrong-unit-count test keeps a TRUE digest so the failure locus is
    exactly the count (asserted: count clause fails, digest clause passes).
    No assertion was weakened; three new tests discriminate the new
    behavior (digest mismatch loud, unknown lock field, malformed block
    locus).
13. **D6 proof location (this session).** Recorded in the test file's
    header: the plan's `tests/proofs/p1_reference_integrity` spelling cannot
    be materialized under the arch suite's cargo-target-path rule; the proof
    lives as a composition-root integration test with the target name the
    gate uses (`--test p1_reference_integrity`). Logged as a
    reference-candidate entry (D8), not an arch-suite change.
14. **The boundary-call helper is named, and boundary 1 is load-bearing
    (this session).** `boundary_verify` is the named helper; `digest_rechecked`
    asserts quietness at boundary 1 (a tripwire firing on an unmoved ruler
    would otherwise pass the detection assertions) and detection exactly at
    boundary 2 (`detected_at == Some(2)`), with the settlement pre-check as
    an explicitly second line of defense.

## Gate output

**No gate was run in this finisher session** (write-only mandate). Nothing
below is pasted output; nothing is fabricated. Runs quoted from earlier
sessions are from `.agents/reports/task7/builder-checkpoints.md`:

- `cargo nextest run -p gauntlet-app reference::` — `[ran]` 2026-08-17 for
  the D1 module (6/6) and D2 module (20/20) as then landed; NOT run since
  D4R/D7 changed `gauntlet-app`. **Orchestrator must run.**
- `cargo nextest run --test p1_reference_integrity` — `[none]` never run.
  **Orchestrator must run.**
- Digest agreement gate
  `diff <(scripts/reference-digest.sh .wayfinder/answer-key.md) <(cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md)`
  — `[none]` never run. Expected value on both sides, read from the frozen
  lock (`.gauntlet/reference.lock`, `[read]`):
  `sha256:ccc7c99be3d3b60488930eed9975dc6d67378a84bf3b13910b48511e9c228859`.
  **Orchestrator must run and paste.**
- `just check && just lint && just deny && just boundaries && just ruler` —
  `[none]` this session. (`just ruler` PASS was observed by the orchestrator
  on 2026-08-18 before this dispatch, per the reconciliation checkpoint.)
- `cargo nextest run --workspace` — `[none]`. **Orchestrator must run.**
- `cargo check -p gauntlet-app` — PASS, orchestrator-run 2026-08-18 BEFORE
  D4R/D5/D6/D7 landed; it does not cover this session's edits.

## Teeth proposals (for the orchestrator to run; none executed here)

- **(a) Boundary-call removal → `digest_rechecked` fails.** Delete the
  `boundary_verify` call inside `digest_rechecked`'s `for boundary in
  2u32..=3` loop (or the whole loop): `detected_at` stays `None` and
  `assert_eq!(detected_at, Some(2), …)` fails. Deleting boundary 1's call
  instead removes the false-positive guard — with it gone, a provider whose
  `verify_unchanged` always fires would still pass the detection asserts, so
  the tooth has two surgical faces.
- **(b) Per-line-trim normalize → determinism tests fire; the diff gate
  alone does NOT (honesty note).** Add `.trim_end()` per line in
  `normalize.rs::normalize`. What fires, deterministically:
  `reference::normalize::normalization_is_sensitive_to_content_edits`
  (the pinned `ALPHA_TRAILING_SPACE_SHA256` constant and the
  `assert_ne!` on `"alpha \nbeta\n"` vs `"alpha\nbeta\n"` both fail) and
  `short_inputs_pin_the_exact_normalized_output` (pinned outputs include
  preserved trailing whitespace and the whitespace-only last line
  `"alpha\n  \n"`). What does NOT fire, verified read-only in this session:
  the script-vs-CLI diff gate on `.wayfinder/answer-key.md` itself, because
  the frozen key contains zero lines with trailing whitespace (`grep -cE '
  $|\t$'` → 0). Proposed gate strengthening (decision for the orchestrator):
  extend the gate with a second diff over a synthetic fixture that DOES
  carry a trailing-space line, so the composed gate catches the trim
  without relying on the ruler's accident of having no trailing spaces.
- **(c) Other teeth seen:**
  - Delete the F4 freeze guard (`let actual = digest_normalized…` block in
    `freeze`) → `provider::a_mutation_between_normalize_and_freeze_fires_the_tripwire_at_freeze`
    fails.
  - Delete the unknown-field check in `read_reference_lock` (or empty
    `LOCK_FIELDS`) → `doctor::an_unknown_field_in_the_reference_record_is_a_finding`
    fails.
  - Make the doctor digest clause pass unconditionally (skip the compare)
    → `doctor::a_moved_answer_key_fails_check_two_as_a_reference_class_failure`
    fails.
  - Make `verify_unchanged` conflate broken artifacts with mutation
    (return `ReferenceMutated` on read failure) →
    `provider::a_broken_artifact_after_freeze_is_unavailable_not_reference_mutated`
    fails.
  - Make the schema clause ignore failures (report pass when
    `parsed.failures` is non-empty) →
    `doctor::a_malformed_block_fails_the_unit_schema_clause_with_its_locus`
    fails.

## Residuals / limits (what this round does NOT claim)

- **AK-023/AK-024 are substrate-level.** The iteration-boundary and
  settlement-pre-check callsites are wired into the controller at plan Tasks
  12–13; `digest_rechecked`'s loop is a harness standing in for them. P1
  here proves the provider/evidence/state composition and the tripwire
  primitive at the composition root — not that the real controller calls
  them.
- **Nothing in this session was compile-checked or test-run** (ESC-002
  write-only mandate). Everything in D4R, D5, D6, D7 is `[read]`-verified
  only. Specific compile-risk spots the orchestrator's `cargo check` should
  watch first: the new `use` lines in `doctor/checks.rs` (items exist:
  `Digest::parse`, `JsonValue::keys`, `defaults::DIGEST_ALGO` all `[read]`),
  the byte-level match arms in `check_reference`, the doctor test
  `lock_text`/`scratch_answer_key` helpers, and the p1 test's adapter API
  usage (`record_run`, `load_run`, `FsEvidenceStore::open`, pin-layout
  paths). `cargo fmt --check` was also not run; lines were hand-wrapped to
  ≤100 columns.
- **The diff gate's trim-insensitivity** (teeth (b) honesty note) is a real,
  documented gap in the composed gate as spelled; the in-module pinned
  tests cover it, and the gate extension is proposed above.
- **Port `normalize` does not surface per-block failures** (F1 deviation,
  decision log 7). On provider paths alone, a key with one malformed block
  yields that Unit missing while the digest still covers the raw bytes; the
  three strict-admission callsites are the compensating controls. If the
  spec owner wants the port itself strict, that is a `CandidateRevision`
  against the suite, not a builder patch.
- **`minimum_reliability_passes` and the two manifest deferrals** remain
  open until the comparison-contract task defines kind/producer mappings
  (decision log 3).
- **AK-025** (candidate revision not adoptable mid-run) is NOT addressed by
  this task; its locator (`tests/antigaming/g2_change_reference`) is the
  same materialization class and was not logged in D8 because the brief
  scoped D8 to AK-021 and AK-022/023/024 only.
- Doctor's manifest clause still checks only top-level mapping shape, not a
  full manifest parse (unchanged from Task 5; the full parser exists in D3
  and is exercised on the provider path).

## Reference candidates

Two dated entries appended to `.agents/reports/reference-candidates.md`
(D8), `[read]`-cross-checked against the file after appending, following
the existing AK-049/AK-054 format, each explicitly marked "additional
instance of the existing locator-materialization class (first recorded
2026-08-17, AK-049)":

1. **AK-021** (`tests/contract/reference::normalization_is_deterministic`)
   → cites `gauntlet-app reference::normalize::…` determinism and
   sensitivity tests plus the script-vs-CLI digest-agreement gate.
2. **AK-022/AK-023/AK-024** (`tests/proofs/p1_reference_integrity…`) → cite
   the three named fns in `gauntlet-cli tests/p1_reference_integrity.rs`
   (`frozen_copy_outside_workspace`,
   `builder_mutation_of_the_active_reference_aborts_with_reference_mutated`,
   `digest_rechecked`) plus `the_run_carries_the_frozen_version` as adjacent
   evidence, with the AK-023/024 substrate-level scope note.

The ruler itself is untouched; `just ruler` is the orchestrator's
confirming run.
