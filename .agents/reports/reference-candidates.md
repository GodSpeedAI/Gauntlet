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
