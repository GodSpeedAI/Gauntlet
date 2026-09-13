# Workbench agent interaction — make the TUI a harness you can talk to

Status: in flight (operator-approved 2026-09-12: "go ahead with all 5").
Branch: `task26-dcritic-apparatus-repair`. Governing specs unchanged: DESIGN.md,
ICM, UESC, the C1 contracts in `workbench/src/planned-operations.ts`.

## Problem

`just tui-dev-up` opens a supervision console. The composer's `prime`/`message`
modes print "unsupported … Message was not sent" (`workbench-host.ts` `submit()`),
there is no way to start a Gauntlet run from the TUI, and the dev recipe always
targets a mock scratch project. The substrate exists (RealPrimeHost, dispatcher,
`gauntlet run`), but nothing connects the operator to it.

## Increments (sequential; each Sonnet-implemented, orchestrator-verified)

| # | Increment | Binds |
|---|-----------|-------|
| P1 | Composer `prime` → `prime.launch`; `message` → `prime.message` (resume/continue a child turn-by-turn, steer when busy); `prime.cancel` → `RealPrimeHost.stopChild`. Composer-launched children run in **work mode**: tools enabled, `--cwd` = project root (T13-bound). | C1 contracts prime.launch / prime.message / prime.cancel |
| P2 | Child sessions + mailbox + assistant output visible and selectable in the Chat/Trace surfaces; selecting a child sets the `message` destination. | UESC "Prime messages = communication, not completion" |
| R  | `run.start` (ICM §13.1, BIND_EXISTING) via the existing canonical CLI (`gauntlet run "<intent>"`) spawned detached through the T13 boundary in the project root; composer `run` mode + palette + MenuBar entry; availability with exact reason; result = admission/run identity, never completion. | ICM §13.1/13.2 |
| D  | `just tui-dev-up project=<path>`: serve a real project; a pidfile-tracked supervisor retries `gauntlet serve` until the first run exists; workbench gets `project_root`/`gauntlet_binary`; no mock run is ever composed in a real project. | justfile |

## Decisions taken under "sensible defaults"

- prime.launch admission keeps the C2 design: canonical admission for the focused
  unit (the one gateway path) precedes spawn. Unavailable → exact reason, zero spawn.
- Work mode enables Prime tools because a coding harness child with `--no-tools`
  cannot edit code. Containment stays process containment (NONGOAL-006: not a sandbox).
  Existing `--no-tools` default is preserved for every existing caller/test.
- No credential files are copied or symlinked. If the isolated agent dir cannot
  authenticate, that is reported, not worked around.
- Closing the TUI never terminates admitted work (REQ-PRIME-009); runs are detached.

## Verification bar (each increment)

Orchestrator re-runs `bun x tsc --noEmit`, full `bun test`, biome on changed files
(baseline: 4 pre-existing findings, 0 new). Final: live tmux drive against a real
project + real prime-agent, captures under `.agents/reports/workbench-agent-interaction/`.
