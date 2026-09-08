# B3 progress — task27 (final conformance + handoff)

Working checkpoint per the charter's progress rule. A claim with NO
settlement authority.

## Phase 1 (landed 2026-09-07) — DONE, awaiting orchestrator re-sweep

- **a. reference-candidates.md** — appended one dated batch entry
  `## 2026-09-07 — Task 27 conformance-sweep divergence batch (eight
  entries covering the seventeen units the committed manifest carried
  as `unrecorded`; task 27 B3, single writer)` with eight `###`
  sub-entries covering all 17 divergences (AK-003; AK-004/AK-006;
  AK-007; AK-008–AK-012; AK-015/AK-017/AK-018/AK-020; AK-059;
  AK-061; AK-064/AK-065), each with: unit + answer-key line range +
  verbatim claim, the frozen locator, the real evidence target
  (nextest spelling + source file), the class citation (AK-049
  locator-materialization class; AK-063 class for the ports-suite and
  skills/formation groups), scope honesty, proposed correction, and
  the batch-closing "not changed by this entry" paragraph. Verified
  beforehand that NONE of the 17 had a dedicated dated entry (grep:
  AK-061's two mentions are the task-20 semantic entries; AK-065's is
  AK-060's class citation).
- **b. conformance-manifest.yaml** — exactly the 17
  `divergence: unrecorded` lines flipped to
  `divergence: reference-candidates 2026-09-07` (git diff: 17
  insertions, 17 deletions, nothing else). Cross-validation proven by
  the CLI tooth test (below).
- **c. scripts/pin-evidence-bundle.sh** — NEW, POSIX sh, executable,
  shellcheck-clean. Pins conformance.json + every file under the
  current sweep's conformance-evidence dir (arg or newest UTC stamp) +
  handoff.md + baselines.json + mutation.json into
  `.agents/reports/evidence-bundle.sha256` (`sha256sum -c` form,
  repo-relative paths). `--check` verifies. Refuses non-zero with a
  clear message on ANY missing input. NOT executed against the real
  tree yet (handoff.md does not exist — phase 2g). Tested in a temp
  fixture: pin OK (auto-newest picks the later stamp), `--check` OK
  from repo root AND from another cwd, tamper ⇒ check FAILED non-zero,
  explicit older dir re-pins its own files, real-tree run refuses on
  missing handoff.md writing nothing.
- **d. Phase-1 battery** (every flock its own top-level command,
  sccache stopped inside the same locked command, package-scoped
  first, no parallelism raised):
  - `cargo nextest run -p gauntlet-cli --test conformance_cli
    the_committed_manifest_agrees_with_the_frozen_ruler` → **1/1 PASS**
    (the flipped manifest cross-validates against the frozen ruler).
  - `cargo nextest run -p gauntlet-arch-tests --no-fail-fast` →
    **245/245 PASS** (incl. single_writer over the edited
    reference-candidates.md, literal scanner, denylist).
  - `just ruler` → **PASS** (ruler never moved).

## STOPPED HERE — waiting for the orchestrator's re-sweep + commit

Phase-2 precondition: updated COMMITTED conformance.json (dated
`reference-candidates 2026-09-07` divergence citations, AK-067 still
INCONCLUSIVE until handoff.md lands).

## Phase 2 — DONE 2026-09-07/08 (under the orchestrator's sequencing
adjustment: intermediate re-sweep skipped; settlement re-sweep
regenerates conformance.json)

- e. `.agents/reports/handoff.md` LANDED (sha256 `b2104e1a…`): six
  category sections 65/4/0/1/0/0, every unit once, AK-067 in proposed
  with its verbatim `no_artifact` reason, UNK-002/UNK-003 terminal
  dispositions in the unknown section, disclosures (no-durable-run
  verified by execution; four bounded bases quoted verbatim;
  mutation-record basis; divergence census). The six-category
  predicate holds — proven by execution (the CLI tooth adjudicates
  AK-067 PASS now).
- f. `crates/gauntlet-arch-tests/src/docs_consistency.rs` (new, 8 unit
  tests) + legs (f)/(g) in `tests/contract/docs_present.rs` + one
  lib.rs mod line. RED-first: leg (f) observed RED (handoff unreadable)
  before the artifact landed, then 7/7 GREEN; tooth A (T4 row-move via
  GAUNTLET_HANDOFF_FIXTURE) RED with 4 violations; tooth B (AK-062
  json downgrade via GAUNTLET_CONFORMANCE_JSON_FIXTURE) RED naming the
  ahead line. Real-artifact digests unchanged across both teeth. 27
  doc claim lines upgraded mechanically (6 stay: the bounded four).
- g. pin EXECUTED: 50 files pinned, `--check` verified.
- h. docs/HANDOFF.md: pointer form kept (dispatch-allowed), stale
  prose fixed, AK-068 line upgraded.
- i. Battery: arch-tests **255/255**, conformance_cli **9/9** (after
  two disclosed updates in B1's file forced by the handoff landing),
  cli_surface **4/4**, fmt+clippy clean on both touched crates, pin
  verified, `just ruler` PASS.
- Completion contract: `.agents/reports/task27/b3-report.md` (includes
  the anticipated settlement-correction playbook and the deviations
  list).
