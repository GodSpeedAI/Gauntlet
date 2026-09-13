# demo-calculator

A tiny, always-available Gauntlet demo project. Its whole job is to give you
something you can start a **real** run against in under a minute.

> **Known limitation (2026-09-13): a run starts, but cannot yet finish.**
> Three real runs against installed `prime-agent` 0.8.0 all ended the same
> way, with no `calculator.html`. The `prime-agent` runner adapter
> (`crates/adapters/gauntlet-adapter-agent-prime`) launches a `--print`
> parent run, which spawns the builder as an rlm child, and it assumes that
> child outlives the parent ("daemon-backed"). In 0.8.0 it does not: when
> the parent's turn ends (~1 minute), the child's in-flight request is
> aborted mid-work (`Request was aborted`). The adapter then waits for a
> result that can never arrive until the stall timeout fires. Its cancel
> sends `prime-agent stop <child>`, which answers `Unknown active session`.
> The adapter treats that as fatal, because its already-gone check matches
> only "No such"/"not found" and never sees stderr. Cost: zero tokens
> settled, no evidence pinned. Everything up to real dispatch works:
> preflight, frozen reference, admission, a real child session with real
> tool calls. The evidence from each run is kept under
> `targets/.runs/demo-calculator/`.

## What this is

- A minimal Gauntlet project (`GAUNTLET.md`, `.wayfinder/`, `.gauntlet/`,
  `.agents/roles/`, `scripts/check.sh`) whose goal is: build
  `calculator.html`, a single-page HTML calculator, with a real check
  command that verifies it actually works.
- `runners.default: prime-agent` — **this dispatches a real agent and
  spends real model tokens.** It is not the mock/replay runner. Budgets in
  `GAUNTLET.md` are kept small on purpose (`max_rounds: 3`,
  `max_wall_seconds: 900`, `max_cost_usd: 2`) but every run against this
  project costs something real.
- No `calculator.html` ships here. Writing it is the point of dispatching
  the agent. `scripts/check.sh` fails cleanly, before any run, because
  there is nothing to check yet.

## How to run it

The easiest path is the repo's `just` recipes, which handle building the
`gauntlet` binary, materializing a working copy of this template at
`targets/demo-calculator` (this directory itself is never touched or
mutated), bringing up `gauntlet serve`, and launching the workbench TUI:

```bash
just demo-up      # sets up targets/demo-calculator + serve + workbench TUI
# ... inside the workbench, start a run against the demo project, e.g.
# via the command palette / composer 'run' mode, or from another
# terminal: gauntlet run "build calculator.html" (cwd = targets/demo-calculator)
just demo-down    # stops what demo-up started and removes targets/demo-calculator
```

`just demo-up` prints the working copy's path and the port it's serving on.
`just demo-down` is idempotent — safe to run any time, including when
nothing is running — and removes only `targets/demo-calculator`, never the
whole `targets/` folder.

## How to run it without the workbench

You can also drive it with the CLI directly, from a scratch copy (never
run `gauntlet` commands against this `examples/` directory itself):

```bash
cp -r examples/demo-calculator /tmp/my-demo-calculator
cd /tmp/my-demo-calculator
gauntlet run "build calculator.html: a single-page HTML calculator with digit and operator buttons and a working evaluation path"
./scripts/check.sh   # verify what the agent produced
```

## Removing the demo

`just demo-down` removes `targets/demo-calculator` and stops the `gauntlet
serve` process (and its supervisor) it started. It never touches this
`examples/demo-calculator` template, and never touches anything else under
`targets/`.
