# Task 6 builder report — round 1 (RECONSTRUCTED 2026-08-17)

**Provenance note:** the round-1 builder session was interrupted after
completing the work and its checkpoint log but before writing this report,
running the final gates, or entering the critic loop (see
`.agents/lessons/session-interruption-recovery.md`, fifth instance). This file
was reconstructed by the orchestrator from `builder-checkpoints.md` plus
independent verification of the tree; the reconstruction adds no claims the
tree does not evidence. The correction round's report
(`correction-round1.md`) is the builder's own document, written as it went.

## Artifacts (as verified from the tree)

- `crates/adapters/gauntlet-adapter-state-sqlite/`: `encode.rs` (canonical
  payloads), `migrations.rs` + `migrations/{0001,0002}.sql` (append-only set,
  applied in order from `schema_migrations`), `event_log.rs` (INSERT/SELECT
  only, `synchronous=FULL`, typed `Damaged`), `state_store.rs` (§16.3
  commit + replay, export_jsonl in 5 ordered segments),
  `bin/crash_helper.rs` (real-process abort instrument),
  `tests/{contract_suite,crash_recovery}.rs`.
- `crates/adapters/gauntlet-adapter-evidence-fs/`: `src/{lib,store}.rs` —
  content-addressed write-once store, fsync+rename publication, pins as
  create-new markers, no overwrite/delete anywhere in the API.
- `crates/gauntlet-app/src/state/{mod,export}.rs`: canonical export service
  (strict parse, canonical shape, total order, duplicate-key refusal, no
  minted timestamps); test module path `state::export::deterministic` per the
  plan's gate filter.
- Recorded acts: `ADAPTER_EXTERNAL_ALLOWLIST` += rusqlite (state-sqlite) and
  sha2 (evidence-fs), pin test updated in the same change; boundaries 190/190.
- Deviations: rusqlite is declared in the ADAPTER manifest only — the root
  `[workspace.dependencies]` form is denied by the denylist (vendor deps
  belong in the adapter's own manifest); deny.toml needed NO licence edit
  (rusqlite tree is MIT/Apache-2.0, already allowed).

## Gates (orchestrator-verified, post-recovery)

`cargo nextest run -p gauntlet-adapter-state-sqlite` 17/17 · evidence-fs 4/4 ·
`gauntlet-app state::export::deterministic` 7/7 · shared contract suites green
via the Task 4 factory registration · `just check/lint/deny` 0 · boundaries
190/190 · `just ruler` PASS · workspace 413/413 (pre-correction counts).

## Teeth (orchestrator-run, each reverted byte-identical after the bite)

- (a) removing the checkpoints `ORDER BY` from the export query → the pinned
  golden test failed on a byte diff (write order ≠ iteration order);
- (b) standing permanent teeth: colliding-digest typed refusal + the
  no-overwrite-API source scan;
- (c) swapping the §16.3 steps in the crash helper's settle script → the
  half-settle test failed on "the half-applied settlement is not yet a row";
- (d) declaring rusqlite in gauntlet-app → two boundary failures (layer
  allowlist + vendor-identifier denylist);
- honest limit: flipping `synchronous` to OFF is NOT demonstrable via process
  abort (the OS page cache survives process death; fsync guards OS/power
  failure, which cannot be injected here) — the pragma configuration is the
  mechanism, recorded as such, echoing critic-A's fsync-proof observation.

## What the interruption lost (recovered by this session)

The final report, the teeth runs, and the entire critic loop. The loop ran
post-recovery: critics A/B (reports alongside this file), adjudication, a
13-item correction round (`correction-round1.md`), and confirmation critic C
(**CONFIRM**, 13/13 closures, 0 new critical/major). See the closure record in
`.agents/CURRENT_STATUS.md`.
