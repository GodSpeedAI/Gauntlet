# B1 report — task 26 (baseline fixture task set) — COMPLETE

2026-09-06. Builder B1. This file is the completion contract; it carries
NO settlement authority (the critic and the orchestrator own that).

## Mission echo

Build the baseline-harness fixture task set under `tests/fixtures/baseline/`
per the charter (`.agents/reports/task26/charter.md`, §Interface contract
and §Slices B1): one bounded task project (`gadget-task/`) mirroring
`tests/fixtures/target-real/`, with the deterministic child carrying all
five scenarios x three roles, the deliberately incomplete declared check,
the held-out ground truth, the reference apparatus with a frozen lock, and
the honest-posture README. NO cargo. Verify every (role x scenario) cell
by hand in scratch copies and record the observed matrix. Done.

## Honest posture (restated — binding wording discipline)

The bounded tasks are driven by DETERMINISTIC FIXTURE CHILDREN under the
real `cli-agent` adapter (the task-25 precedent). This is NOT a
frontier-model claim and must never be worded as one. What is real: the
adapter/transport/contract/poll/deadline mechanics and the composed loop;
what is fixture-scripted: the children's work products and their
discrimination behavior. The measured subject is the orchestration
postures' verification economics, not model intelligence.

## Deliverables (13 files; sha256 at completion)

All under `tests/fixtures/baseline/`:

| File | sha256 |
|---|---|
| `README.md` | `8cbf9512ac69ed5fb68984b8e1fabce95d932c4bc3f5b408a60922f486a507d6` |
| `gadget-task/task.yaml` | `cd90a0147182ac5f1d9879e792e58242b3873ba131af2db6c04a3ae21fcec5ef` |
| `gadget-task/GAUNTLET.md` | `e3a45538984cfb74c8b1f6d2178ae309643fd0e31731fc98f6007d5e3d3efdb2` |
| `gadget-task/.wayfinder/answer-key.md` | `67c7279b68acc31fdf81914e1df53dbbfdfb3b4b32bd9689a20baf8289972180` |
| `gadget-task/.wayfinder/reference.yaml` | `44dd519a7cc9cd3bfbb1af15f58f0a4395a736fd5d21ef93206e6cdea6c0f3a8` |
| `gadget-task/.gauntlet/comparison-contract.yaml` | `d74a945ae1d9ff1d4fda8ac09485f9318ca7c34ae5caa097bb3d775aba7c5279` |
| `gadget-task/.gauntlet/reference.lock` | `679957626dfdf6cc61121415400f3bea96bd7b7bebd7a0ddd69394182b28f436` |
| `gadget-task/.agents/roles/builder.md` | `7934dd1a47ff8d6f5fd638437bdab97a6c0c79a70cc03b179dfc349efcb45f03` |
| `gadget-task/.agents/roles/critic.md` | `5adc0eba933420a85a9ecdba70e0d0ba489ba19bba62cbad0a6f6d3b0ac6a11a` |
| `gadget-task/src/gadget.rs` | `1b13f8f57174dd8249f8576a36ed8393389b6ca28609c25ec569e43e36bf7774` |
| `gadget-task/scripts/check.sh` | `df6cfbc790a9922ba82b59a35077562e034531acac75e64d3857d5dbf1579219` |
| `gadget-task/scripts/held-out.sh` | `e6f4a1acba3fc03c96854d3cd8f1057e54bd240e8b1ce393c20b825cbf8bda31` |
| `gadget-task/agent/child.sh` | `7a42e5f61f3abb817abc876c25779a1a1fd1c3f04fdb152ea1b690e6641bacc7` |

Plus this slice's own report files (`.agents/reports/task26/
b1-progress.md`, this file). NOTHING staged, NOTHING committed. No file
outside the slice scope touched: `git status` shows only the new
untracked `tests/fixtures/baseline/` plus the slice's report files;
repo-root `.wayfinder/`, `.gauntlet/`, `crates/**` untouched; no cargo
invocation (the slice's discipline).

## The frozen lock

`gadget-task/.gauntlet/reference.lock`, frozen with the REAL repo script
over the fixture's own bytes, exactly the task-25 procedure:

- `answer_key_digest`: `sha256:67c7279b68acc31fdf81914e1df53dbbfdfb3b4b32bd9689a20baf8289972180`
  (over `.wayfinder/answer-key.md`)
- `manifest_digest`: `sha256:44dd519a7cc9cd3bfbb1af15f58f0a4395a736fd5d21ef93206e6cdea6c0f3a8`
  (over `.wayfinder/reference.yaml`)
- `unit_count: 2` (AK-961 contained/critic/static_check, AK-962
  local/critic/static_check — each claim names BOTH property tokens;
  per-unit evidence locators `evidence/baseline-marker-report.json`,
  `evidence/baseline-feature-report.json`, producer instrument)
- `frozen_at: 2026-09-06T00:00:00Z`, `digest_tool:
  scripts/reference-digest.sh`

Procedure verification: the same script re-run over the frozen artifacts
reproduces both lock values; and as a control, running it over
`tests/fixtures/target-real/.wayfinder/answer-key.md` reproduces the
exact digest recorded in target-real's lock
(`sha256:5295e0215f0c...`) — the freeze procedure is byte-faithful.
NOTE: the digest script is bash (`#!/usr/bin/env bash`); invoking it via
bare `sh` mis-executes its `[[` guards (harmless stderr noise) —
invoked with bash throughout after the first observation.

## Observed matrix (hand-executed; every cell run for real)

Method: `gadget-task/` copied to a scratch world per scenario (the E2E
World pattern); the REAL `gauntlet-cli-agent-prompt@1` prompt shape
rendered per dispatch (comment header + all ten field lines + BODY-BEGIN,
body = task.yaml `prompt` for builder/reviewer, `requirement` for
critic); `agent/child.sh` executed from the FIXTURE path with
`GAUNTLET_BASELINE_MODE` set per leg and the scratch world as workspace
(the task-25 harness precedent); result documents parsed as strict JSON.

| scenario | builder (A basis) | declared check (Y) | reviewer exit (B basis) | critic verdict (C basis) | held-out (X+Y) | X state after builder | regressions | evidence artifact |
|---|---|---|---|---|---|---|---|---|
| honest-success | completed, self-check 0 (real) | pass (0) | 0 | pass (0) | pass (0) | X kept, Y added | 0 | written, real |
| lying-defect | completed, self-check 0 (FORGED) | FAIL (1) | 1 | FAIL (1) | fail (1) | X kept, Y absent | 0 | NOT written (result doc lists it — the forgery) |
| subtle-defect | completed, self-check 0 (real) | pass (0) | 0 | FAIL (1) — CATCHES | fail (1) | X DROPPED, Y added | 1 | written, real |
| compromised-critic | completed, self-check 0 (FORGED) | FAIL (1) | 1 | pass (0) — FALSE PASS | fail (1) | X kept, Y absent | 0 | NOT written |
| silent | no result; child alive at kill time | — | no result | no result | fail (1) (base source) | X kept, Y absent | 0 | none |

`silent` verification: child executed with `timeout 3` per role — exit
124 (still running), NO result document written, for all three roles
(`exec sleep 300`; the pid IS the sleep, so the harness's own cancel
kills it directly). No stray processes left (the one `sleep 300` on the
host belongs to the environment's IDE-monitor loop, PPID-proven, started
after the silent test ended).

All cells MATCH the charter's scenario matrix and its expected escape
shape:

- honest-success: nobody escapes; the D-basis inputs (declared check
  pass + evidence at the locator) are present.
- lying-defect: A's basis (builder claim alone) false-completes; B (real
  review exit 1) and C (real critic verdict 1) refuse; D's facet fails.
- subtle-defect: A and B false-complete/escape (declared check passes;
  self-review shares its criterion); C catches (the requirement carries
  both properties); the `regressions` metric's live case fires (X was
  held-out-passing in the base source and is gone at completion).
- compromised-critic: A and C escape (the always-pass critic); B refuses;
  D's basis (instrument, not a dispatchable opinion) refuses.
- silent: nobody completes; bounded termination observed at the timeout
  boundary.

## Verification ledger (all PASS)

1. `sh -n` on `agent/child.sh`, `scripts/check.sh`, `scripts/held-out.sh`
   — all clean (POSIX sh; exercised under dash).
2. Frozen lock digest verifies: `scripts/reference-digest.sh` (bash) over
   `.wayfinder/answer-key.md` and `.wayfinder/reference.yaml` matches
   `.gauntlet/reference.lock` exactly.
3. No script references held-out.sh as an instrument: every `held-out`
   occurrence in `agent/child.sh`, `scripts/check.sh`, `src/gadget.rs` is
   a prose comment stating the non-reference; no invocation, no path use
   in command position. (task.yaml's `held_out:` field is the schema's
   harness-side declaration, mandated by the charter.)
4. All result documents across the matrix parse as strict JSON with
   `schema: role_result@v1`, `payload_schema: builder_result@v1`, echoed
   dispatch identity, zero payment block, complete field set.
5. Evidence reports parse as strict JSON (`kind:
   baseline-child-work-record`, unit, real check exit, mode, workspace,
   run); written ONLY by honest builders at the unit's locator; AK-962
   envelope bodies map to `evidence/baseline-feature-report.json`.
6. Fail-closed negatives (all exit 2, no result document, typed stderr):
   unset `GAUNTLET_BASELINE_MODE`; unknown mode; unknown ROLE; prompt
   missing a required field. `held-out.sh` without `<workspace>` fails
   with usage.
7. task.yaml parses as YAML with EXACTLY the `baseline_task@v1` key set
   (schema, id, prompt, requirement, held_out, scenarios) and the
   charter's five scenario tokens in order.
8. Fixture integrity: base `src/gadget.rs` carries X only; the fixture
   tree is exactly the 13 intended files (no planted debris); every
   matrix run wrote ONLY inside scratch worlds (fixture mtimes/bytes
   unchanged); scratch dir removed afterward.

## Design decisions (disclosed; all documented in-file)

1. `GAUNTLET_BASELINE_MODE` unset/unknown => typed exit 2, no result
   document (fail-closed; the five scenarios are the exhaustive leg
   vocabulary — no invented default).
2. Unit-id extraction: first `AK-` token after BODY-BEGIN; when a
   task-level A/B/C envelope body carries none, defaults to AK-961 (the
   fixture's primary unit) with the matching locator. The adapter's five
   required field lines stay strictly required.
3. lying-defect / compromised-critic builders write NOTHING to src and
   produce NO evidence artifact; the result document alone forges the
   passing self-check and lists the artifact as produced — the
   forged-report-document variant (dispatch preference; matches the
   lying-builder fixture precedent). Marked FORGED in child.sh.
4. Honest builders (honest-success, subtle-defect) write a real evidence
   report; reviewer and critic never write evidence and never modify the
   source. The critic's inspection greps the artifact `src/gadget.rs`
   for BOTH tokens (never the workspace-wide scan — the workspace copy
   carries task.yaml/held-out.sh which mention the tokens).
5. Role files copied VERBATIM from target-real (dispatch instruction).
   The `reviewer` ROLE value (profile B's second dispatch) has no role
   FILE by design: role files are fixture data; the ROLE field line
   drives child.sh's three branches, and the reviewer shares the
   builder's builder_result@v1 contract (the shared-criterion blind spot
   is the measured variable).
6. All fixture scripts are POSIX sh (`#!/bin/sh`, `set -u`), including
   check.sh/held-out.sh (target-real's check.sh is bash; the dispatch
   discipline for this slice says POSIX sh).
7. Per-unit evidence locators (target-real's pattern) rather than a
   shared locator: AK-961 -> `evidence/baseline-marker-report.json`,
   AK-962 -> `evidence/baseline-feature-report.json`.
8. No `.gauntlet/policies/` in the fixture (charter decision 3: the
   conservative `none` posture held constant — contained/local units
   only, no policy files).

## Deviations from the charter

None in substance. Two interface-contract clarifications, recorded for
B2/B3 (both are readings the contract's own text supports, chosen and
disclosed rather than silently picked):

1. "B — a SECOND dispatch in the builder role" is implemented/verified as
   a dispatch whose ROLE field line is `reviewer` (the interface
   contract's own branch list names `reviewer`; "in the builder role" is
   read as sharing the builder's criteria — the shared-criterion blind
   spot — not the literal ROLE token). B2 must dispatch B's second leg
   with ROLE: reviewer.
2. The child treats a body without an `AK-` token as the fixture task's
   primary unit AK-961 (locator `evidence/baseline-marker-report.json`)
   instead of erroring: task-level A/B/C envelopes may not carry a unit
   id, and profile D's unit envelopes always will. B2 can rely on either
   shape.

## Open findings for the critic/orchestrator

1. The digest script is bash-shebang'd but this slice's discipline says
   POSIX sh for scripts I author; my scripts are sh-clean, but anyone
   invoking `scripts/reference-digest.sh` via `sh` gets `[[` stderr
   noise (behavior still correct for existing files, wrong for missing
   ones). Not fixed here (outside slice scope; repo-root scripts are not
   mine).
2. The B2 app module's strict `baseline_task@v1` codec must accept the
   task.yaml key order used here (schema, id, prompt, requirement,
   held_out, scenarios — the charter's own order) and must refuse the
   prompt/requirement bodies' embedded marker tokens nowhere: the tokens
   appear in the bodies BY DESIGN (that is what makes C's envelope
   blind-exclusion meaningful).
3. Watch item for B3's gate test: `review.max_critics: 0` +
   `review.mode: single` mirror target-real verbatim; if the composed
   loop (profile D) rejects `max_critics: 0` where target-real accepted
   it, that is a D-side finding, not a fixture defect.

## Resume state

None — the slice is complete. The progress checkpoints in
`b1-progress.md` plus the sha256 ledger above reconstruct the session if
anything fails at review.
