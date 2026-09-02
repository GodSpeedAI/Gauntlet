# Task 20 / Builder B2a brief — six-category report service

You are Builder B2a for plan Task 20 in /home/sprime01/projects/gauntlet.
Read first, in order: `AGENTS.md` (repo root), `.agents/reports/task20/charter.md`
(decisions D1–D10 and guardrails bind you), then this brief. Your slice is
B2a only — a single NEW application module. Do not touch the CLI, the
controller, govern, adapters, or E2E fixtures (other slices own those).

## Goal

Plan Task 20 step 3: "Report generator distinguishing implemented-and-proven
/ provisional / configured-but-untested / proposed / blocked / unknown," and
the plan's done-when: "the report's status categories are populated from
actual records rather than hardcoded (AK-060, AK-061)."

Create `crates/gauntlet-app/src/report/` (module + `pub mod report;` in
`crates/gauntlet-app/src/lib.rs` — that one line is your only edit to an
existing file). Design it as a pure read-side service over the durable
records:

- Input: a run id + the ports it needs (`&dyn StateStore`, and whatever read
  APIs already exist — INVENTORY FIRST: grep the `StateStore` port
  (`crates/gauntlet-ports/src/state_store.rs`) and the sqlite adapter for
  read methods (runs, units/iterations/invocations/residuals/records,
  escalations, budget). PREFER existing reads and `export_jsonl` composition
  over adding port methods. If a needed read truly does not exist, you may
  add ONE targeted read method to the port + its implementors + contract
  suite — but only if composition from existing reads is impossible; record
  the choice.
- The six categories, exactly these spellings (plan §Task 20 step 3;
  the spec §26 settlement criteria and the handoff vocabulary use the same
  set): `implemented-and-proven`, `provisional`, `configured-but-untested`,
  `proposed`, `blocked`, `unknown`.
- Population rule: a category is populated FROM RECORDS (unit statuses,
  verification records, settlement records, escalation rows, event log) —
  never from string literals claiming capability. A unit with a settlement
  record + verification evidence is implemented-and-proven for the run; a
  unit whose claim was verified `INCONCLUSIVE`-blocked or awaiting evidence
  lands per its actual status; units never dispatched because of budget/
  governance are `blocked`; unattempted units with no record are `unknown`
  (run-scope). Capability-scope rows (e.g. which §20.1 commands are wired)
  come from a caller-supplied manifest structure, NOT invented inside the
  report module — the CLI slice (B2) will own declaring that manifest from
  its real command table. Keep the module honest: it reports what records
  say; it does not know what code exists.
- Output: a typed report struct with a canonical machine rendering (JSON via
  the repo's existing serialization conventions — check how doctor/state
  export serialize) AND a concise human summary. Both from the same struct.
- Tests (in `crates/gauntlet-app/tests/report.rs` or in-module, matching
  repo conventions): (1) a run with settled+verified units populates
  implemented-and-proven from records; (2) removing the settlement record
  moves the unit out of implemented-and-proven (teeth: the generator reads
  records, not intentions); (3) blocked/unknown populations are record-
  driven; (4) the machine rendering is deterministic for identical records
  (byte-identical twice); (5) no category is populated without a
  corresponding record (hard assertion over an all-empty store ⇒ all
  categories empty, exit-neutral).

## Boundaries

- Read-only over stores: the report service writes nothing.
- No new dependencies. No edits outside `crates/gauntlet-app/src/report/`,
  the one `lib.rs` line, and your test file.
- Compile discipline: `flock .agents/reports/task20/compile.lock -c '<cmd>'`
  around every cargo invocation (B1 runs in parallel — expect brief waits).
  Prefer `just check-package gauntlet-app` / `just test-package gauntlet-app`.

## Checkpoint protocol (mandatory)

`.agents/reports/task20/b2a-progress.md`: declare-first `STARTED`/`DONE`
lines per item with evidence (commands, test counts). Your final message may
be lost — the progress file is your contract. Finish with
`.agents/reports/task20/b2a-report.md`: what landed, the exact public API
surface you exposed (B2's builder composes against it), test counts, teeth
evidence, deviations.

## Done when

`just check-package gauntlet-app` and `just test-package gauntlet-app` pass
including your new tests, `just lint` passes, `just boundaries` and
`just ruler` pass.
