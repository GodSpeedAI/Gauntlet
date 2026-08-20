# Task 9 — orchestrator teeth record (2026-08-20)

All four teeth were run by the orchestrator personally, each: break →
watch the designed test fail → revert byte-identical → confirm green.
No builder was involved. The tree after all four is byte-identical to the
finisher's D6 state (workspace 605/605, boundaries 192/192, lint exit 0,
ruler PASS — the post-teeth run in this file's transcript).

Note for replay: the Task-9 files are untracked until the task commit, so
`git checkout --` cannot restore them; every revert was a manual inverse
edit (T2 needed three: struct field, `of_bundle` population, `to_json`
emission).

## T1 — delete the `unavailable_evidence` population (AK-030 / §21.3)

- Break: removed `unavailable.push(missing);` from the single population
  site (`crates/gauntlet-app/src/observation/assembly.rs`, the
  `gap_entries_for` consumer — the one-line deletion the plan's done-when
  names).
- Failures observed:
  - `gauntlet-app observation::assembly::tests::observer_failures_populate_both_gap_arrays_exactly_once`
  - `gauntlet-app observation::assembly::tests::an_observer_store_write_failure_surfaces_as_gap_entries`
  - `gauntlet-cli g6_hide_failed_observations a_failed_probe_is_recorded_in_both_arrays_and_covered_by_the_digest`
  - `gauntlet-cli g6_hide_failed_observations a_probe_store_write_failure_surfaces_as_gap_entries`
- Revert: inverse edit; `observation::` 23/23, g6 3/3 green.

## T2 — break the structural claims exclusion (AK-029 / I2 / P3)

- Break: the plan's own named adversary — "someone later adds a helpful
  context field". Added `claims_context: String` to `VerificationInputs`,
  populated it in `of_bundle` from `doc.claims_facet().content()`, emitted
  it in `to_json`.
- Failures observed:
  - `gauntlet-app observation::input::tests::claims_text_never_reaches_verification_inputs`
  - `gauntlet-app observation::input::tests::the_facet_and_gap_inputs_are_identical_under_a_claims_swap`
  - `gauntlet-cli p3_real_observation the_lying_builders_claim_diverges_from_reality_inside_the_bundle`
  (the proof itself — the lying builder's claim would influence a
  verification input.)
- Revert: three inverse edits; `observation::` 23/23, p3 2/2 green.

## T3 — drop the per-facet env fingerprint from the evidence encoding (AK-028 / §12.2)

- Break: `FacetEvidenceDoc::encode` wrote an empty `env_fingerprint`
  string in place of the computed one.
- Failures observed:
  - `gauntlet-ports observer_evidence::tests::encode_decode_round_trip_is_lossless`
    (the strict decoder refuses an empty fingerprint — the schema has
    teeth of its own)
  - `gauntlet-ports observer_evidence::tests::wrong_shapes_are_typed_errors`
  - `gauntlet-adapter-observe-filesystem shared_contract a_facet_carries_full_provenance_and_matching_evidence`
  - `gauntlet-adapter-observe-filesystem shared_contract the_manifest_is_deterministic_and_never_follows_symlinks`
- Revert: inverse edit; ports observer 9/9, filesystem 4/4 green.

## T4 — include timestamps in the digest normalization (§12.4)

- Break: removed the `"observed_at" => None` arm from
  `observation::digest::normalized_bytes` (bundle-level timestamp back in
  the digested bytes).
- Failures observed:
  - `gauntlet-app observation::digest::tests::shifted_timestamps_do_not_change_the_digest`
  - `gauntlet-app observation::digest::tests::normalized_bytes_is_the_full_json_minus_the_timestamp_members`
- Revert: inverse edit; `observation::digest` 4/4 green.
