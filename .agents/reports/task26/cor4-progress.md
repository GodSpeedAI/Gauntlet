# COR-4 progress (task 26 corrections round 4)

Builder: COR-4. Binding inputs: `falsifier-round1.md` (F1–F7), `charter.md`
(incl. adjudications), the dispatch brief's COR-4 adjudications.

## Design decisions (as built)

- **Durable source of test-facet executions (F1 leg 2)**: the leg run's
  per-iteration observation bindings (`StateStore::observation_binding`)
  resolved through the leg's own evidence store (`FsEvidenceStore` over
  `context.resolved.evidence_dir`) to the bound `observation_bundle@v1`
  documents; the count is the number of `kind == "test"` `FacetRecord`
  entries across every bound bundle. One test-kind record == one test-facet
  execution == one wrapper invocation == one snapshot entry (the wrapper
  writes exactly one entry per execution: `.rs`, `.absent`, or
  `.copy-failed`). Named in code at the read site.
- **Nonce shape**: per-leg ULID (executor-issued, in-process at wrapper
  materialization), baked into the wrapper's stamp as
  `<cwd-basename>-<nonce>-<nanostamp>-<pid>` so existing gate-test snapshot
  assertions (prefix `<basename>-`, suffix `.rs`/`.absent`) hold unchanged.
  An entry is IN the nonce set iff its name contains the leg's nonce.
- **Grading decision order (run_held_out, D path)**: (1) wrapper digest
  re-verify (pinned at materialization, executor-owned in-process) →
  mismatch = tamper-INCONCLUSIVE, nothing executed; (2) nonce/count
  cross-check → any non-nonce entry, or nonce-set count != durable count
  (unless 0 == 0) = tamper-INCONCLUSIVE; (3) 0 == 0 → disclosed leg-copy
  fallback (unchanged); (4) else judge snapshots with the F5 exit contract.
- **F5**: `held_out_reading_from_exit` (app module): 0 = pass, 1 = fail,
  other = inconclusive; spawn failure = inconclusive; `false_completion`
  becomes `Option<bool>` (null on inconclusive); `held_out_basis` gains
  `judge-channel-tampered` and `held-out-instrument-error`.
- **F3**: codec refuses `id` that is not a single safe path component
  (typed error with line); executor containment check canonicalizes the
  materialized leg dir against the scratch root (defense in depth).
- **F4 + F1 leg 3**: two new `_meta` disclosure consts in the app module
  (`instrument_runs` denominators; the none-posture substitution boundary).

## Log

- [initial] Read falsifier report, charter, baseline module (mod/task/
  harness/metrics/report), improve_cmd.rs executor, gate test, fixture
  child/check/held-out, state-store/observation/evidence surfaces.
  Durable source located; design above recorded.
- F3a codec validation: RED-first proven — tests written against the new
  `TaskError::IdNotASafeComponent` with the validation stubbed to `Ok(())`;
  both tests RED (`a_traversal_id_is_refused_as_an_unsafe_component`,
  `backslash_leading_dot_and_dotdot_ids_are_refused`), then implemented,
  then GREEN.
- F5 app module: `HeldOutReading` three-state + `held_out_reading_from_exit`
  exit contract + `HeldOutBasis::{JudgeChannelTampered, InstrumentError}` +
  `false_completion: Option<bool>` + the two new `_meta` disclosures
  (`instrument_run_denominator_disclosure`, `judge_channel_boundary_disclosure`).
  gauntlet-app: 627/627 green (620 prior + new/changed).
- F1/F3b CLI: `JudgeChannel` (wrapper path + pinned sha256 + snapdir +
  per-leg ULID nonce + durable facet-execution count); wrapper stamps
  `{nonce}` between the cwd basename and the nanostamp (gate-test T5
  snapshot-name assertions hold unchanged); `grade_judge_channel` order:
  digest → nonce set → count cross-check → fallback → judge;
  `durable_test_facet_executions` reads observation bindings through
  `FsEvidenceStore` and decodes the bound `observation_bundle@v1`
  documents (test-kind `FacetRecord` count); `ensure_leg_containment` in
  `materialize_workspace`; held-out helpers rewritten over the F5 exit
  contract (spawn failure / missing instrument / out-of-band exits are
  INCONCLUSIVE, never run-aborts or substantive fails).
  CLI unit teeth: 7/7 green. improve_baseline suite: 8/8 green — the gate
  test's adjudicated matrix assertions hold UNCHANGED on the honest
  fixture (digest pin + nonce set + durable-count equality all hold on
  every honest D leg), plus the F1 wrapper-tamper and F2 plant tests
  through the real binary and the F3 traversal refusal.
- Next: mutation proofs (digest / nonce / count / exit-band / containment),
  then arch tests, full battery, scratch gate shape, verify-ruler, report.

## Final verification (all under the top-level flock protocol)

- arch: 237/237 (one loop: the literal scanner caught 33 new literals in
  my code; fixed by structure — Component-based id guard, runtime-assembled
  attack strings, `instrument_script`-built script bodies, a shebang-
  preserving hook — never by weakening the scanner).
- gauntlet-app: 627/627 (620 prior + 7 new).
- gauntlet-cli: 149/149 (139 prior + 10 new; 2 slow: plant test 64 s, gate
  test 73 s). fmt clean; clippy clean (one needless-borrow in a test
  cleanup line, fixed).
- Real gate shape into scratch (`/tmp/cor4-gate/baselines.json`, exit 0):
  charter matrix unchanged, all 10 `_meta` disclosures, every D leg basis
  present (audit in cor4-report.md).
- `./scripts/verify-ruler.sh`: PASS. No flake observed in any battery run
  (the recorded `b1_contract_wins_over_exit7` watch item did not fire).
- Mutation-proof note: the M1–M5 proofs ran against the pre-fmt text and
  restores were byte-identical at proof time (sha256 pairs below); a
  `cargo fmt` pass and the arch-literal restructure followed, whitespace-
  level for the mutated loci (the mutated comparison lines are unchanged),
  and the post-restructure battery is green. Recorded plainly rather than
  re-proven per mutation.
- Evidence: /tmp/cor4-teeth/{sha256-before.txt,sha256-after.txt,
  cli-battery.log,cli-battery2.log}; scratch worlds under /tmp/cor4-gate
  (gate shape, retained) — remove with `rm -rf /tmp/cor4-gate
  /tmp/cor4-teeth`. No git add/commit; fixtures/frozen paths untouched.
