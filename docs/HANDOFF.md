# Handoff

**The canonical handoff report is `.agents/reports/handoff.md`** — the
frozen locator the ruler's AK-067 names. That report is the single source
of per-unit truth, together with the conformance sweep output at
`.agents/reports/conformance.json`. This file is a pointer, never a second
handoff: category counts, unit lists, and every `INCONCLUSIVE` unit with
its reason live there, not here.

## The six status categories (spec §26)

The canonical report distinguishes:

| Category | Count |
|---|---|
| implemented_and_proven | see `.agents/reports/handoff.md` |
| implemented_but_provisional | see `.agents/reports/handoff.md` |
| configured_but_untested | see `.agents/reports/handoff.md` |
| proposed | see `.agents/reports/handoff.md` |
| blocked | see `.agents/reports/handoff.md` |
| unknown | see `.agents/reports/handoff.md` |

Counts and memberships are mechanically derived from
`.agents/reports/conformance.json` and asserted equal by the task-27
consistency test; a count written here would be a competing source of
truth (which the spec §25 forbids).

The standing operator questions also live in the canonical report's
unknown section: UNK-003 (which concrete runner `runners.default` should
name — the operator's standing decision; the repo root deliberately names
none) and UNK-002 (whether this repository should carry a LICENSE, and
which — the operator's call).

## Where each kind of truth lives

- Per-unit outcome and status: `.agents/reports/conformance.json` and
  `.agents/reports/handoff.md` — nowhere else.
- Trust posture, sandbox tiers, profile adjudication, advisory-only
  enforcements, UNK-001: `docs/CONFORMANCE.md`.
- Per-task build evidence: `.agents/reports/task*/closure.md` and
  `.agents/CURRENT_STATUS.yml`.
- Recorded locator divergences and ruler-owner questions:
  `.agents/reports/reference-candidates.md`.

## Proven vs provisional

Status categories are spec §26's six; unit-level claims defer to
`.agents/reports/conformance.json`. The canonical report named above is
this same task's wave-2 deliverable; until it exists, nothing here claims
anything on its behalf.

- implemented_but_provisional — AK-068: the eight-file §25 documentation
  set exists and `tests/contract/docs_present` enforces its structure —
  all eight files present and non-empty, the status section required in
  each, the canonical handoff locator named here, the binding
  CONFORMANCE.md content, and the six-word status vocabulary on every
  unit-level claim (this task's B2 slice; RED-first teeth recorded in
  `.agents/reports/task27/b2-report.md`).
