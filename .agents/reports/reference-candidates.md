# Reference candidates — dated entries against the frozen ruler

Per `.gauntlet/reference.lock`: ".wayfinder/answer-key.md is never edited for
any reason … A Unit that seems wrong becomes a dated entry in
`.agents/reports/reference-candidates.md` with evidence." The same discipline
applies to the spec source (`.agents/specs/Core-spec.md`): revisions go
through the normal spec process, never a builder's direct edit.

## 2026-08-17 — AK-049 evidence locator names an unmaterializable test target (Task 5 correction K8 / critic-B R8)

- **Unit:** AK-049 (`.wayfinder/answer-key.md` lines 341–346; identically
  `.agents/specs/Core-spec.md` lines 1745–1749).
- **Claim under review (unchanged, satisfied):** "A sandbox policy file for
  the irreversible tier causes doctor to fail."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/e2e/doctor::irreversible_policy_is_error`; no such test exists
  (`tests/e2e/` holds only `.gitkeep`), and the architecture boundary
  suite's Cargo-target-path rule (`gauntlet-arch-tests` `escapes.rs`
  `scan_cargo_target_paths`, exemption list exactly
  `["crates/gauntlet-arch-tests"]`) forbids a core crate from declaring a
  repo-level test target, so the locator cannot be materialized as spelled.
- **Proposed correction:** re-declare the locator to the in-crate test paths
  that prove the claim (all in `crates/gauntlet-app`):
  - `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run`
    (canonical spelling `irreversible.json` in the effective dir: check 5
    fail, AK-049 in summary and doctor.json, exit non-zero, removal green);
  - `doctor::tests::forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`
    (K4 hardening: `irreversible.yaml`, `IRREVERSIBLE.JSON`, extensionless
    `irreversible`, and a broken symlink all fail; clean dir green);
  - `doctor::tests::a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`
    (K4 hardening: forbidden file in the default `.gauntlet/policies` dir
    fails with both-dir evidence while `sandbox.policy_dir` is overridden).
- **Live-CLI corroboration:** planted `.gauntlet/policies/irreversible.json`
  → `just doctor` exit 1, check 5 fail citing AK-049; removed → exit 0
  (Task 5 teeth, re-run after the K4 hardening in the 2026-08-17 correction
  round; transcript in `.agents/reports/task5/builder-round1.md`).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (digests still match; `just ruler` green). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-17 — AK-054 evidence locator names an unmaterializable test target (Task 6, critic-A MINOR-7)

- **Unit:** AK-054 (`.wayfinder/answer-key.md` lines 376–381).
- **Claim under review (unchanged, satisfied at the instrument level):** "Run
  records export deterministically to JSONL; exporting the same run twice
  produces byte-identical output."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/state::deterministic_export`; no such file or target exists,
  and the architecture boundary suite's Cargo-target-path rule (same class as
  the AK-049 entry above) forbids a core crate from declaring a repo-level test
  target, so the locator cannot be materialized as spelled.
- **Proposed correction:** re-declare the locator to the tests that prove the
  claim today:
  - `gauntlet-app state::export::deterministic::export_is_byte_identical_and_canonical_under_reordered_input`
    (the plan's own Task 6 gate filter; byte-identical double export under a
    store that reorders lines between exports);
  - `gauntlet-adapter-state-sqlite state_store::tests::export_is_the_pinned_golden_and_byte_identical`
    (pinned golden bytes, out-of-order checkpoint writes);
  - `gauntlet-cli tests/export_stack.rs the_real_sqlite_export_stack_is_deterministic_and_canonical`
    (the composed stack: real adapter bytes through the canonical re-export,
    twice, byte-identical).
- **Not changed by this entry:** the answer key, the spec, and the frozen lock
  (digests still match; `just ruler` green after every round). Settlement of
  this candidate belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-021 evidence locator names an unmaterializable test target (Task 7, deliverable D8)

- **Unit:** AK-021 (`.wayfinder/answer-key.md` lines 142–148).
- **Claim under review (unchanged, satisfied at the instrument level):**
  "Reference resolution normalizes deterministically: the same logical
  reference always produces the same digest."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/reference::normalization_is_deterministic`; no such file
  or target exists, and the architecture boundary suite's
  cargo-target-path rule (same class as the AK-049 entry above) forbids a
  core crate from declaring a repo-level test target, so the locator cannot
  be materialized as spelled. This is an **additional instance of the
  existing locator-materialization class (first recorded 2026-08-17,
  AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the tests that prove
  the claim today (in-crate, under the plan's own gate filter
  `cargo nextest run -p gauntlet-app reference::`):
  - `gauntlet-app reference::normalize::normalization_is_deterministic_across_line_ending_and_final_newline_spellings`
    (one digest across CRLF/LF/lone-CR endings and final-newline
    present/absent/doubled spellings — AK-021's determinism face);
  - `gauntlet-app reference::normalize::normalization_is_sensitive_to_content_edits`
    (the teeth face of the same rule: one trailing space on a content line,
    one interior blank line, and reordering two lines each change the
    digest);
  - corroborating composed-path evidence: the Task 7 gate diffs
    `scripts/reference-digest.sh .wayfinder/answer-key.md` against
    `gauntlet reference digest --path .wayfinder/answer-key.md` and
    requires byte-identical output — two independent implementations of the
    same rules agreeing is the determinism proof.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this write-only builder session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-022/AK-023/AK-024 evidence locators name an unmaterializable test target (Task 7, deliverable D8)

- **Units:** AK-022 (`.wayfinder/answer-key.md` lines 149–155), AK-023
  (lines 156–163), AK-024 (lines 164–170).
- **Claims under review (unchanged):** AK-022 — "The frozen reference is
  stored in the evidence store outside every builder-writable path.";
  AK-023 — "A builder attempt to modify the active reference fails or is
  detected and aborts the run with reference_mutated."; AK-024 — "The
  reference digest is re-verified at every iteration boundary and
  immediately before settlement."
- **Defect (evidence declaration only):** the three locators are spelled
  under `tests/proofs/p1_reference_integrity…`; no such repo-root target
  can exist under the same cargo-target-path rule as the AK-049 entry
  above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** —
  not a new defect class.
- **Proposed correction:** re-declare the locators to the composed-path
  proof at the composition root, under the target name the plan's gate
  already uses (`cargo nextest run --test p1_reference_integrity`;
  `crates/gauntlet-cli/tests/p1_reference_integrity.rs`, with its
  location-rationale header comment):
  - AK-022 → `gauntlet-cli tests/p1_reference_integrity.rs frozen_copy_outside_workspace`;
  - AK-023 → `gauntlet-cli tests/p1_reference_integrity.rs builder_mutation_of_the_active_reference_aborts_with_reference_mutated`;
  - AK-024 → `gauntlet-cli tests/p1_reference_integrity.rs digest_rechecked`;
  - adjacent composed-path evidence: `the_run_carries_the_frozen_version`
    (freeze binds the `ReferenceVersion` to run identity through the real
    embedded state store).
- **Scope honesty:** AK-023/AK-024 remain substrate-level until plan Tasks
  12–13 wire the real controller iteration boundaries and the settlement
  pre-check; `digest_rechecked`'s harness loop stands in for those
  callsites, and the provider module header names the three real
  strict-admission callsites.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this write-only builder session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of these candidates belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-025 evidence locator names an unmaterializable test target (Task 7 correction C4, critic-A MAJOR-1)

- **Unit:** AK-025 (`.wayfinder/answer-key.md` lines 171–176).
- **Claim under review (unchanged):** "A run may record a candidate reference
  revision but cannot adopt one mid-run."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/antigaming/g2_change_reference`; no such repo-root target can exist
  under the same cargo-target-path rule as the AK-049 entry above. This is an
  **additional instance of the existing locator-materialization class (first
  recorded 2026-08-17, AK-049)** — not a new defect class. Recorded now
  because the Task 7 round's D8 deliverable covered AK-021 and AK-022/023/024
  and stopped one Unit short of the plan's done-when range (critic-A MAJOR-1).
- **Disposition honesty (both halves of the claim):**
  - "cannot adopt one mid-run" — substrate-level today: the P1 tripwire
    (`gauntlet-cli tests/p1_reference_integrity.rs`
    `builder_mutation_of_the_active_reference_aborts_with_reference_mutated`
    and `digest_rechecked`) aborts on any mid-run reference move, and the
    frozen `ReferenceVersion` makes adoption structurally impossible without
    a new version. The runtime-level "record but never adopt" flow (a
    `CandidateRevision` admitted against the frozen ruler) is the anti-gaming
    suite's business.
  - "may record a candidate reference revision" — no substrate yet; the
    domain `CandidateRevision` type exists (Task 3) but no run-path records
    one. Owned by plan Task 22/23 (memory/optimizer, P9/P10) and exercised
    adversarially by plan Task 24 (G2).
- **Proposed correction:** re-declare the locator when Task 24 materializes
  the anti-gaming suite (its tests live under a crate test target per the
  same rule), citing the then-existing G2 test plus the P1 tripwire tests as
  the substrate evidence.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` PASS verified by the orchestrator in the same session).
  Settlement of this candidate belongs to the ruler's owner, not the builder.
