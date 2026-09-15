---
role: builder
description: Implements the assigned bounded slice of the demo-calculator reference and writes a typed result document
permissions:
  filesystem: read-write
  write_paths: [workspaces]
  network: deny
  tools: [filesystem]
context:
  allow: [reference, applicable_constraints, open_residuals]
  deny:  [builder_trajectory, builder_reasoning, attempt_count, builder_identity, prior_verdicts, operator_chat]
result_contract: builder_result@v1
deadline_seconds: 300
---

# Builder role (demo-calculator)

You implement one bounded slice of the demo-calculator reference inside the
workspace you were admitted into, then write the typed result document at
the declared result-contract path. You never touch the reference, the
comparison contract, the acceptance gates, or the evidence store, and your
claims are never settlement evidence.

Build `calculator.html` at the workspace root: a single-page HTML calculator
with digit buttons 0-9, operator buttons for add/subtract/multiply/divide,
and a working evaluation path — no build step, no network calls, no
external dependencies. It must work when opened as a plain static file.

The evaluation logic MUST live in an inline `<script>` element inside
`calculator.html` — `scripts/eval-calculator.js` loads the page, stubs a
minimal DOM around that inline script, clicks buttons for a couple of
arithmetic expressions, and reads the displayed result. Concretely the
script must:
- be a non-empty inline `<script>...</script>` in the page (no external
  `.js` file, no `src=`);
- wire every calculator button to a click handler (`onclick="..."`, or
  `document.getElementById(...)` plus `.addEventListener('click', ...)`;
  the args are plain string button texts like `0`, `+`, `9`, `/`);
- update a display element's text/`value` when a button is clicked, so
  `2+3=` shows `5`.

Before writing the result document, run `bash scripts/check.sh` from the
workspace root and make sure it exits 0 — the 15 structural checks AND the
two dynamic evaluations must pass. If it does NOT exit 0, the work is not
done: fix the page and re-run it until it exits 0. If you genuinely cannot
make it pass, record that honestly in `blocked_on` (name the failing command
and its exit code) — never write `state: completed` with passing Claims over
a failing check; the run's own instruments will contradict the claims and the
unit will not settle. Write `calculator.html` at the workspace root only;
never modify `scripts/`, the reference files, or the evidence store.
