# Task 9 — round 1 report (finisher-after-interruption)

Two sessions produced this round: the round-1 builder (D1–D3 verified
green at checkpoint time, then D4 code landed but UNPROVEN when a usage
limit killed the session) and this finisher (proved D1–D3 still green,
proved and finished D4, did D5, ran all D6 gates). The checkpoint file
(`builder-checkpoints.md`) is the slice-by-slice contract; this file is
the round-level report for the critic and orchestrator.

## Artifact inventory

| Artifact | State at finisher start | Finisher action |
|---|---|---|
| `crates/gauntlet-ports/src/observer_evidence.rs` (D1) | landed, green 7/7 | re-verified; encode/validation refactored schema-driven for the boundary reconciliation (bytes unchanged) |
| five `crates/adapters/gauntlet-adapter-observe-*/` (D2) | landed, green 4+5+3+3+5 = 20/20 | re-verified; untouched (fmt only) |
| `crates/gauntlet-app/src/observation/` (D3) | landed, green 23/23 | re-verified; facet serializers refactored schema-driven; test kind-literals → `FacetKind::token()` |
| `crates/gauntlet-cli/tests/p3_real_observation.rs` (D4) | landed, NEVER RUN (binary had been compiled by the dying session; no run, no checkpoint) | first-ever run 2/2 PASS; audit against the brief: nothing weaker; clippy-driven production fix (below); fmt |
| `crates/gauntlet-cli/tests/g6_hide_failed_observations.rs` (D4) | landed, NEVER COMPILED | first compile surfaced an unused-import warning (a clippy `-D warnings` failure) → fixed; 3/3 PASS; fmt |
| `crates/gauntlet-cli/Cargo.toml` dev-deps (D4) | landed | verified correct (five observe adapters + mock runner, dev-only) |
| `crates/gauntlet-arch-tests/src/deps.rs` (D5) | untouched | adapter-layer `sha2` REASON extended to name the observe adapters' use (env fingerprints + filesystem manifest digests, plan Task 9); no entries/pins changed |
| `.agents/reports/reference-candidates.md` (D5) | untouched | three entries appended (AK-028/029/030, locator-materialization class, real test names) |
| `crates/adapters/gauntlet-adapter-agent-mock/src/deterministic_clock.rs` | untouched since Task 8 | PRODUCTION FIX (see below): `Cell` → `AtomicU64` interior + discriminating concurrent test |
| `crates/gauntlet-arch-tests/src/{walk,denylist}.rs` | untouched since Task 6 | deliberate, recorded boundary-suite growth (see below) |

## Per-gate counts (final, post-fmt, this finisher's runs)

```
cargo nextest run --test p3_real_observation        2/2
cargo nextest run --test g6_hide_failed_observations 3/3
cargo nextest run -p gauntlet-app observation::     23/23  (filter non-empty)
cargo nextest run -p gauntlet-ports observer        9/9
cargo nextest run -p gauntlet-adapter-observe-filesystem  4/4
cargo nextest run -p gauntlet-adapter-observe-git         5/5
cargo nextest run -p gauntlet-adapter-observe-build       3/3
cargo nextest run -p gauntlet-adapter-observe-test        3/3
cargo nextest run -p gauntlet-adapter-observe-command     5/5
cargo nextest run -p gauntlet-adapter-agent-mock         69/69  (68 + finisher's new tooth)
just check        clean
just deny         advisories ok, bans ok, licenses ok, sources ok
just boundaries   192/192  (190 + 2 new finisher pins)
just ruler        PASS (frozen lock untouched)
cargo nextest run --workspace   605/605
just lint         exit 0 (clippy --workspace --all-targets -D warnings + fmt --check)
```

## Production-code fixes the finisher had to make (for the critic brief)

1. **`DeterministicClock`: `Cell` → `AtomicU64` interior** (mock adapter,
   a closed-Task-8 crate). Clippy `arc_with_non_send_sync` (×2) in p3's
   `Arc::new(DeterministicClock::…)` — the repo's canonical deterministic
   clock could not participate in the port's `Arc<dyn Clock>` wiring that
   every composition (p3 now, Task 12 later, the `GAUNTLET_CLOCK` hook)
   requires. Observable semantics preserved exactly (pure `now`,
   monotonic-max `sleep_until` via compare-exchange, snapshot `Clone`
   kept). Discriminating test:
   `deterministic_clock::tests::concurrent_sleeps_settle_on_the_maximum_target_under_sharing`
   (a `Cell` interior cannot compile it). 69/69 in the crate.
2. **Boundary reconciliation (the round's headline — full detail in the D6
   checkpoint).** `just boundaries` was RED on the D1–D3 code (92 hits):
   Task 9's core codecs must spell B.3-family wire keys (`"command"`) the
   Task-2 denylist masks only inside role_result.rs. Resolution: honest
   test-side kind-token spellings; schema-driven (zip) serializers so key
   literals live only in declared field tables and `.get()` positions;
   then a DELIBERATE, RECORDED, PINNED growth of the wire-key mask —
   `ANNEX_B_CONTRACT_PARSER` → `ANNEX_B_CONTRACT_PARSERS` (4 codec
   modules), `"command"`'s key annotation extended to B.3, and the
   array-element mask shape extended to rustfmt's line-initial element
   form (previous-line context, residual documented). All three mask
   gates (module, declared key, position) preserved; selection-shaped
   literals still report everywhere, pinned by the new/extended tests
   (`annex_b_contract_parsers_are_pinned`,
   `annex_b_wire_keys_are_masked_in_the_task9_codec_modules_too`,
   extended corners). No scanner evasion anywhere (no concat/format!
   spellings, no allow).

## Fail-first evidence personally observed by the finisher

- g6's FIRST-EVER compile: unused-import warning (live red at clippy
  level) → removed → green.
- clippy over p3: `arc_with_non_send_sync` ×2 → atomic-interior fix →
  clean (red→green).
- `just boundaries`: 92 violations → reconciliation → 192/192 (the
  largest live red→green of the round).
- `just lint`: one `manual_contains` in the new predicate → `.contains`
  → exit 0.
- D3's T1/T4 teeth were run LIVE by the round-1 builder (recorded in its
  D3 checkpoint: population-site deletion and timestamp pass-through both
  failed their tests, reverted byte-identical).

## Teeth readiness (finisher did NOT run them — orchestrator's)

- **T1 (gap population single site):** `gap_entries_for` in
  `crates/gauntlet-app/src/observation/assembly.rs` — the ONLY place an
  `ObserverError` becomes both §12.3 entries; deleting either push fails
  `observer_failures_populate_both_gap_arrays_exactly_once` AND g6's
  `a_failed_probe_is_recorded_in_both_arrays_and_covered_by_the_digest`
  (composed level).
- **T2 (structural claims exclusion):** delete the type separation (e.g.
  add a `From<ClaimsFacet> for FacetRecord` or read `claims_facet` in
  `VerificationInputs::of_bundle`) — there is no such path; the canary
  tests (`input::claims_text_never_reaches_verification_inputs`, p3's
  per-claim sweep) fail only if one is CREATED, and
  `a_claims_facet_claiming_inclusion_does_not_decode` pins the wire flag.
- **T3 (env fingerprint):** remove the fingerprint computation or empty
  the field — each adapter pins
  `the_env_fingerprint_is_sorted_pairs_over_sha256`; p3 asserts the
  evidence's fingerprint non-empty; bundle decode rejects an empty
  `environment_fingerprint`.
- **T4 (timestamp exclusion):** break `normalized_bytes`' timestamp
  removal → `shifted_timestamps_do_not_change_the_digest` and p3's
  byte-identical re-assembly fail.

## Honest limitations (for the critic)

- The gap records name the facet-kind token, not the instrument name
  (ObserverError carries no instrument; documented in the assembler).
- git's own version is never probed (wrapper-uniformity; recorded D2).
- Digest normalization pins `observation_digest` to `""` (documented,
  pinned); timestamps are epoch seconds (documented B.3 narrowing).
- The wire-key mask's line-initial element shape cannot see an array's
  purpose (residual documented in the predicate doc); it is module-gated
  to the four codec modules and key-gated to declared wire keys.
- The dying builder never ran fmt; the finisher's `cargo fmt` reformatted
  12 landed files (content-neutral; all gates re-run green).
- D3's recorded deviation stands: the brief's "inputs EQUAL under claims
  swap" was unsatisfiable as written (the digest is an input by the
  brief's own field list); the satisfiable property (facets/gaps/bundle_ref
  equal, bytes differ only in the digest identifier) is implemented with
  an in-test comment.
