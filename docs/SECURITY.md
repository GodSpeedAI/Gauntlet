# Security and operational safety

Spec §17 (governance) and §22 (security) are normative. The trust posture
and sandbox-tier adjudication live in `docs/CONFORMANCE.md`; this document
states the safety rules of the shipped runtime. Per-unit conformance truth
lives in `.agents/reports/conformance.json` and
`.agents/reports/handoff.md`.

## Non-negotiable rules

1. **The grader is not writable by the graded.** Answer key, frozen
   reference, evidence store, and event log sit outside every
   builder-writable path. The answer-key surface is single-writer by
   construction: the production modules that name it are pinned by an arch
   scan, and every production write-API token in the core crates is
   inventoried with reason and containment guard
   (`tests/contract/architecture_boundaries`).
2. **Authority gates whether something runs; containment gates how
   contained it is if it does.** Both precede builder process start. An
   `irreversible` unit is never an automatic dispatch candidate at the
   scheduler, regardless of any prompt. An agent may never lower a Unit's
   blast radius or authority; only an operator raises either.
3. **No unsandboxed fallback.** Sandbox policy validation precedes
   execution; a validation failure escalates. A runner reporting
   `isolation: none` is refused for any tier above `contained`.
4. **Destructive operations** require `GAUNTLET_ALLOW_DESTRUCTIVE=1` *and*
   an authority decision; either alone is insufficient (spec §22.8).
5. **Secrets by indirection only** (`$VAR`): presence validated without
   printing; never written into logs, prompts, envelopes, evidence, or
   generated files (spec §22.4).
6. **Model output is untrusted external input**: parse, validate against
   the declared schema, then admit into consequential state — never before
   (spec §21.2). Result documents carry `invocation_id` and envelope-digest
   binding; mismatches are typed failures, not coerced defaults.
7. **Hooks are trusted configuration**, run inside the workspace with a
   required timeout and truncated output. **Untrusted repositories** run
   with `GAUNTLET_ALLOW_NETWORK=0`, a real containment backend, and
   short-lived read-only credentials (spec §22.6–22.7).

## Workspace safety invariants (spec §17.5)

Builder and critic processes run only inside their assigned workspace
path; paths stay inside `workspace.root` after canonicalization with
symlink escape checked; workspace keys are sanitized to `[A-Za-z0-9._-]`;
every builder invocation above `contained` is wrapped by the tier's sandbox
from process start (the typed `CommandContainment` seam, task 25).

## Recorded boundaries (read before trusting a stronger claim)

- **The kernel is a control environment, not a security sandbox** (spec
  Annex C.2). Containment is the runtime's job; see the trust posture in
  `docs/CONFORMANCE.md`.
- **The hostile-child judge-forgery boundary (task 26).** Under the shipped
  `none` posture a hostile same-user child can forge judge material; the
  shipped defenses are tamper evidence, not prevention, and a privileged
  judge channel (uid or sandbox separation) is the recorded structural
  prerequisite for any hostile-child claim
  (`.agents/reports/reference-candidates.md` entries 6–7).
- **Host readability inside the sandbox (task 14, R-4).** The baseline
  read-only bind makes the whole host filesystem readable inside the
  sandbox, including the frozen ruler — literally spec-conformant, escalated
  to the operator as a CandidateRevision-class question, not closed by this
  plan.
- **Advisory-only enforcements are enumerated** — with their recorded
  triggers — in `docs/CONFORMANCE.md` §"Advisory-only enforcement"
  (G9 critic write-scope; F6 cli-agent run_id echo; the task-26 boundary
  above).

## Failure posture

Typed errors carry the failure locus (spec §21.1); uncertainty is never
converted into success; `PASS`, `FAIL`, and `INCONCLUSIVE` stay distinct
through every layer. Degradation rules (spec §21.3): config/reference
invalid stops new dispatch but keeps reconciliation running; agent session
failure retries or escalates and never settles the unit; sandbox failure
escalates and never falls back; observation failure records unavailable
evidence and dependent verification becomes `INCONCLUSIVE`; integrity
violation aborts the run and preserves everything.

## Proven vs provisional

Status categories are spec §26's six; unit-level claims defer to
`.agents/reports/conformance.json`.

- implemented_and_proven — AK-022: the frozen reference is stored in
  the evidence store outside every builder-writable path
  (`crates/gauntlet-cli/tests/p1_reference_integrity.rs`, task 7; composed
  thereafter in every settlement path).
- implemented_and_proven — AK-023: a builder attempt to modify the
  active reference aborts the run with `reference_mutated` (same proof,
  both digest implementations agreeing; the mock runner carries the
  reference-mutation scenario fixture).
- implemented_and_proven — AK-043: the runtime is the sole writer of
  Unit status and evidence fields — the g10 proof plus the arch-level
  answer-key single-writer scan
  (`crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs`;
  `architecture_boundaries::answer_key_single_writer`).
- implemented_and_proven — AK-044: an `irreversible` unit is never an
  automatic dispatch candidate at the scheduler (p5; re-evidenced through
  the real binary at task 25 — the unit stands in the escalation queue with
  a durable ticket and no dispatch).
- implemented_and_proven — AK-047: sandbox policy validation precedes
  execution and a validation failure escalates without falling back to
  unsandboxed execution (p5 + the real-runner E2E governance leg).
- implemented_and_proven — AK-048: containment wraps the builder
  session from process start — the typed `CommandContainment` seam in the
  CLI adapter, wrap between admission and process start on every
  policy-carrying dispatch, with the child's own inside-sandbox confinement
  artifact bound to the dispatched unit (task 25; the task-20 deferral
  retired RED-first).
