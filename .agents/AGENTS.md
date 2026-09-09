# .agents/ — Agent Workbench Guide

Durable operational memory, task state, normative specifications, and evidence for Gauntlet agents. Follow root `AGENTS.md` for repository-wide invariants and verification gates.

## 1. Directory Roles & Authority

* **`specs/` (Normative Source of Truth)**: Authoritative specifications (`Core-spec.md`, `gauntlet-tui-prime-workbench.spec.yaml`). Code and tests must conform to these. When changing a public contract, persisted schema, architecture boundary, or proof level, update the spec and ADR in the same change.
* **`plans/` (Execution Scratch)**: Tactical plans and task decompositions (`gauntlet-impl.md`, `gauntlet-tui-prime-workbench.plan.yaml`). Plans are non-normative execution aids; do not treat plans as specifications.
* **`roles/` & `agents/`**: Canonical role definitions (`orchestrator.md`, `builder.md`, `critic.md`, `verifier.md`, `optimizer.md`) and subagent prompts.
* **`reports/` (Immutable Evidence)**: Task reports, audit logs, and conformance settlements (`conformance.json`, `handoff.md`, `evidence-bundle.sha256`). Historical evidence is immutable; never rewrite, delete, or fabricate run evidence.
* **`lessons/` (Durable Learning)**: Verified, reusable project lessons. Follow the Open Knowledge Format (OKF) schema defined in `.agents/lessons/AGENTS.md` (YAML frontmatter + 5-section body).
* **`research/` (Preregistrations)**: Research instruments and preregistrations (`.prereg.yaml`, `.prereg.sha256`). Never modify a frozen preregistration or its digest after work begins; verify with `just prereg-check <task>`. Record post-run findings in `.outcome.yaml`.
* **`skills/`**: Procedural workflows and harness cheatsheets.

## 2. Handoff Contract & State Tracking

Every agent session ending with repository changes must update the handoff state before completing:

* **`CURRENT_STATUS.yml`**: Primary machine-readable handoff ledger. Maintain execution state, active task, completed work, verification evidence, remaining steps, blockers, and observations so any agent can resume cold.
* **Dual update rule**: At every dispatch, correction round, and interruption recovery, update both task-local `IN-FLIGHT.md` and `.agents/CURRENT_STATUS.yml`; neither substitutes for the other.

## 3. Workbench Memory Ledgers

* **`OBSERVED_DEBT.md`**: Concrete out-of-scope debt, gaps, risks, or defects noticed while working. Each entry requires evidence, concrete impact, and suggested next move. Update existing entries rather than duplicating.
* **`LESSONS.md`**: Verified, reusable project lessons specific to Gauntlet. Record only lessons backed by executable evidence, commands, or tests.
* **`OPEN_QUESTIONS.md`**: Unresolved choices that genuinely require human judgment and cannot be settled from repository evidence. Always include a recommendation, options, and impact.

## 4. Workbench Invariants

* **One home per fact**: Reference canonical sources; do not copy specifications into scratch notes or duplicate ledgers.
* **Never fabricate evidence**: Only record commands that were actually executed and results that were observed.
* **No conversational transcripts**: Keep memory files concise, structured, and free of chat transcripts or machine-specific absolute paths.