# Task preregistrations — research instrumentation (non-normative)

Nothing in this directory is normative. It does not change the Core spec, the
frozen answer key, the frozen reference, task acceptance criteria, task gates,
task closure rules, reference candidates, or implementation behaviour. It is a
record of what we believed *before* a task ran, kept so that later reports
cannot silently reconstruct our prior expectations after seeing the result.

## Files, two per task

- `task-N.prereg.yaml` — written **before** the task starts; immutable thereafter.
- `task-N.prereg.sha256` — SHA-256 of the exact preregistration bytes, frozen with it.
- `task-N.outcome.yaml` — created/filled **only after evidence exists**.

Templates: `TEMPLATE.prereg.yaml`, `TEMPLATE.outcome.yaml`.
Structured records are YAML, never Markdown.

## Freeze and check

```bash
sha256sum task-N.prereg.yaml | awk '{print $1}' > task-N.prereg.sha256   # once, at freeze
just prereg-check N                                                      # any time after
```

`just prereg-check` recomputes the hash, compares it to the frozen one, fails
loudly on mismatch, and never modifies the preregistration. It is intentionally
independent of `just ruler` and is not part of `just ci`.

## Rules

- Never edit a frozen `.prereg.yaml` or its `.sha256` after implementation begins,
  even if a preregistered belief turns out to be wrong. Preserving the wrong belief
  is the point of the instrument.
- Record everything learned later in the `.outcome.yaml`.
- Preregister what is **visible now**. Do not brainstorm every imaginable failure;
  an artificially comprehensive prereg cannot be discriminating.
- Not every observation is evidence for or against a prediction — `non_discriminating`
  is a first-class result.

## Evidence levels for the transfer criterion

- **L1 replay** — the exact known defect/regression is rejected.
- **L2 parametric variation** — the same known structural failure with changed
  payload/values is rejected.
- **L3 structural transfer** — a previously untested bypass expressing the same
  causal failure class is rejected by the generalized mechanism, *without* adding
  a special-case rule for that exact bypass.

Reaching L3 is never claimed in advance.
