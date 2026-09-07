# B1 progress — task 26 (baseline fixture task set)

2026-09-06. Builder B1. Slice scope: `tests/fixtures/baseline/**` ONLY,
plus this slice's own report files. NO cargo; no commits; nothing outside
the slice scope touched.

## Checkpoints

- [2026-09-06] Charter + prior art read: charter.md (binding), target-real
  tree (GAUNTLET.md, child.sh, check.sh, answer-key, reference.yaml,
  comparison-contract, reference.lock, roles), task-25 b1-report (lock
  freeze procedure: digests via scripts/reference-digest.sh over the
  frozen artifacts, JSON lock record), adapter prompt writer
  (crates/adapters/gauntlet-adapter-agent-cli/src/prompt.rs — real
  gauntlet-cli-agent-prompt@1 field-line order confirmed).
- [2026-09-06] Wrote the full fixture tree (13 files) under
  tests/fixtures/baseline/; froze .gauntlet/reference.lock with real
  scripts/reference-digest.sh digests (procedure control-checked against
  target-real's frozen lock value — byte-faithful).
- [2026-09-06] Hand-verified every (role x scenario) cell in scratch
  copies: observed matrix MATCHES the charter; fail-closed negatives all
  exit 2; strict JSON result/evidence documents confirmed; silent stays
  alive with no result (timeout-killed, no stray processes);
  sh -n clean; held-out never referenced by child/check/src; task.yaml
  parses with exactly the baseline_task@v1 key set.
- [2026-09-06] Scratch cleaned; completion contract written to
  b1-report.md (with the observed matrix, sha256 ledger, deviations,
  open findings). SLICE COMPLETE — no settlement authority.

## Decisions so far (disclosed, none silent)

1. `GAUNTLET_BASELINE_MODE` unset or unknown => child exits 2 (fail-closed,
   typed stderr message). No silent default: the five scenarios are the
   exhaustive leg vocabulary; defaulting could mask a harness bug.
2. Unit-id extraction is best-effort: first AK- token after BODY-BEGIN;
   when the envelope body carries none (task-level A/B/C envelopes may),
   the child defaults to AK-961 (the fixture task's primary unit) and the
   matching evidence locator — documented in child.sh, README, report.
3. lying-defect / compromised-critic builder: writes NOTHING to src AND no
   evidence artifact; the result document alone forges the passing claims
   and lists the artifact as produced (the forged-report-document variant,
   per dispatch preference — matches the lying-builder fixture precedent).
4. Honest builders (honest-success, subtle-defect) write the evidence
   report at the unit's locator; lying builders do not. Reviewer and
   critic never write evidence and never modify src.
5. Role files builder.md/critic.md copied VERBATIM from target-real
   (dispatch instruction). reviewer shares the builder role's
   builder_result@v1 contract (fixture role FILES are data; the ROLE
   field line drives child.sh's three branches).
6. All fixture scripts POSIX sh (`#!/bin/sh`, `set -u`) per dispatch
   discipline (target-real's check.sh is bash; mine is sh).
