# Task 10 interruption-recovery audit

**Date:** 2026-08-20  
**Scope:** read-only reconciliation of the Task-10 in-flight tree. This audit does not
accept builder checkpoint claims as proof.

## Ruler and reconstructed state

- `just ruler` — **PASS**: frozen answer-key and manifest digests match
  `.gauntlet/reference.lock`.
- `git log --oneline -15` ends at `a39c3cb` (Task 9/status), with no Task-10 commit.
- `git status --short` shows the Task-10 implementation as uncommitted changes/new
  paths, matching `CURRENT_STATUS.md`'s explicit Task-10 **IN-FLIGHT** line. It also
  contains unrelated pre-existing `.jolli/`, status, reference-candidates, and Task-9
  changes; this audit attributes only the compare/envelope paths below to Task 10.
- `.agents/reports/task10/` contains only `builder-brief.md` and
  `builder-checkpoints.md`: there is **no** promised `builder-round1.md`.

## Checkpoint-to-tree reconciliation

| Slice | File evidence present | Audit state |
|---|---|---|
| D1 | domain comparison change; ports comparator/`comparison_record`; role-result/contract/stubs; arch changes; compare-model manifest/source | Landed, but checkpoint-only green claim is not independently proven. |
| D2 | `.gauntlet/comparison-contract.yaml`; `gauntlet-app/src/compare/{contract,envelope,mod}.rs`; app wiring | Landed, independently unproven. |
| D3 | `gauntlet-app/src/compare/aggregate.rs` with deterministic aggregation test | Landed, independently unproven. |
| D4 | `gauntlet-app/src/compare/review.rs` with mode/truncation tests | Landed, independently unproven. |
| D5 | compare-diff and compare-model adapter sources | Landed, independently unproven. |
| D6 | untracked `crates/gauntlet-cli/tests/p2_context_isolation.rs` exists (two tests: structural proof and `teeth`), but no D6 checkpoint or final report | **Earliest incomplete slice.** |

The source/test paths and checkpoint mtimes support the interruption pattern: the
checkpoint ends after D5 at 15:05, while the P2 test is newer (15:09). `git diff --check`
returned 0, but that is whitespace-only evidence.

## Commands actually run and results

```text
just ruler                                      PASS
 git status --short; git branch --show-current; git log --oneline -15
                                                main; Task-10 tree uncommitted; no Task-10 commit
 git diff --check                               exit 0
 pgrep -af 'cargo|nextest|rustc'                active: cargo-nextest nextest run -p gauntlet-ports
```

I did **not** start a compiler/test command: the last command showed the parent-owned
ports gate in progress, and the repository's single-compiler rule prohibits concurrent
Cargo/nextest work. Thus no checkpointed green result has been independently reproduced
by this audit yet.

## Recovery point and cheapest proof contract

Resume at **D6 proof/gates**, not at implementation, once the active ports run has
finished. Treat D1-D5 as *landed pending independent verification*, not settled. Run
sequentially (do not overlap builds):

```bash
cargo check --workspace --all-targets
cargo nextest run -p gauntlet-domain comparison::
cargo nextest run -p gauntlet-ports
cargo nextest run -p gauntlet-app compare::
cargo nextest run -p gauntlet-adapter-compare-diff
cargo nextest run -p gauntlet-adapter-compare-model
cargo nextest run --test p2_context_isolation
```

Expected gates: every command exits 0; `p2_context_isolation` runs both the honest
serialized-envelope blindness/instrument-reality proof and the self-contained AK-027
poisoned-field tooth (the tooth passes only by observing `assert_blind` reject each
injected deny-listed field); app compare tests include deterministic shuffled aggregation,
review-mode/budget-gap behavior, and consumption-boundary digest refusal; ports/adapter
runs establish strict codec/contract and both adapter paths. Only after these results is
recorded evidence sufficient to write D6's checkpoint/final builder report and proceed to
the required teeth/critic loop.
