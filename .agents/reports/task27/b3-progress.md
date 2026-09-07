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

## Phase 2 — pending

- e. `.agents/reports/handoff.md` (six §26 categories from
  conformance.json's `status_category`; INCONCLUSIVE units + reasons;
  per-category counts; UNK-002/UNK-003 terminal dispositions in the
  unknown section; disclosures: no durable run to export, four bounded
  units' bases quoted, mutation-record basis). Must satisfy
  `predicates.rs handoff_six_categories` exactly.
- f. consistency test (handoff↔conformance set+count equality with
  RED tooth via fixture; docs claim-line cross-check with env-var
  override to fixture json; then MECHANICAL upgrade of behind-evidence
  doc claim lines).
- g. execute scripts/pin-evidence-bundle.sh for real.
- h. docs/HANDOFF.md placeholder counts → canonical pointer.
- i. battery + b3-report.md.
