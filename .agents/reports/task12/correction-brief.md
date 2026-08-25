# Task 12 — Correction-round brief (C1–C9)

You are a correction builder in /home/sprime01/projects/gauntlet. Read
`AGENTS.md`, then the two blind critic reports
(`.agents/reports/task12/critic-a-result.md`,
`.agents/reports/task12/critic-b-result.md`) and the orchestrator's
adjudication (`.agents/reports/task12/correction-adjudication.md` — the
authoritative disposition of every finding; implement its FIX-NOW items
C1–C9 exactly, record its LATER-SCOPE items L1–L4 in code docs/ledger
comments where the adjudication says so, and do not touch REJECTED items).
You MAY read all task12 reports (correction builders are not blind).

## Method (mandatory)

- **Fail-first per item**: before fixing each C-item, write/adjust the
  discriminating test that fails against the current defect (the critic
  reports cite exact file:line — use them), observe the failure, then fix.
  Record each fail-first observation in the checkpoints file.
- **Checkpoint after EACH C-item**: append to
  `.agents/reports/task12/correction-checkpoints.md` (sessions die here).
- Result contract: `.agents/reports/task12/correction-round1.md` — per-item
  what changed, files, test names, fail-first evidence, gates.
- Update `.agents/reports/task12/teeth.md` if any teeth mechanism moved
  (C2 deletes the proof-local meter supplement — the T1/T4 teeth must
  still bite against the NEW production wiring; re-run and re-record them).

## The items (adjudication is authoritative; this is the working list)

- **C1 terminal truth**: `blocked` vs `budget_exhausted` vs escalation per
  the scheduler's typed Ineligible reasons; no idle/governance-block
  mislabeled as exhaustion.
- **C2 rounds unification**: the run-`Budget` is the single rounds
  authority (iteration-boundary booking); exhaustion ⇒ terminal
  `budget_exhausted` + close-out checkpoint + reserve intact (never a bare
  `ControlError` from the loop's own charge); the rounds gate reads the run
  budget; p6 passes under the BARE `MeteredBudget` (delete
  `RoundsSupplementedMeter`); p4 factory aligned; meter/rounds split
  documented in both module docs.
- **C3 diagnosis composition**: remove `implementation_shaped` /
  `disconfirmed` hardcodes; bounded per-phase spend; `CorrectionIndicated`
  reachable composed; apparatus loci input-driven.
- **C4 composed non-convergence**: attribution fallback to the iteration's
  unit when `unit_refs` is empty (ground in §13.5/§11.5); `ResidualKey`
  carries expected/observed locators; composed recurrence→N→diagnosis test
  through the REAL diff comparator.
- **C5 real selection**: residual-driven path enumerates real candidates,
  scores per §15.1, records selection WITH rejected alternatives;
  `CorrectionSelection::new` refuses empty rejected lists (unrepresentable
  vacuity); authority/tier from real unit facts; cross_cutting completions
  do not error the run.
- **C6 scheduler wiring**: over-capacity scope refusal ⇒ skip-and-continue;
  full membership in `PlanInput.units` for graph validation with separate
  eligibility filtering (no `UnknownDependency` after settlement); cohort
  charge/prompt consistency; §11.3 cohort composition or the honest
  spec-awaiting record (L4).
- **C7 full per-tick re-read**: file authoritative for queued units too —
  project the whole re-parsed set against durable state, not only
  in-flight.
- **C8 checkpoint fidelity**: real `event_log_offset` (store-returned);
  checkpoint at the iteration boundary (post compare/verify); resume
  restores per-unit attempts from durable state.
- **C9 small closes**: `Reservation` construction meter-gated (no public
  forging of parallelism accounting — follow the B1 minting pattern);
  escape escalation-evidence JSON; no silent result drop on charge refusal
  (record + typed disposition); bounded drain (max polls per tick).

## Gates (run personally, one at a time, record outputs)

```
just check-package gauntlet-app && cargo nextest run -p gauntlet-app control::
cargo nextest run --test p4_corrective_loop --test p6_budget_control
just check && just test && just lint && just deny && just boundaries && just ruler
```

Boundaries must stay 197/197 (or grow only through recorded acts); the plan
gate must stay green. Red gate at session end ⇒ report loudly, never weaken.

## Hard rules

Same as all Task-12 slices: never edit `.wayfinder/**`, `.agents/specs/**`,
`.agents/plans/**`, `AGENTS.md`, `justfile`, `deny.toml`,
`.agents/CURRENT_STATUS.md`, frozen prereg files, `.agents/prompts/**`,
`.tmp/**`, `.jolli/**`; no commits; single-compiler lock; named-const
literal idiom; typed errors; no weakened assertions — if a critic finding
cannot be fixed without weakening something, STOP and record the conflict
in the checkpoints + report instead.
