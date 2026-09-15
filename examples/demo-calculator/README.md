# demo-calculator

A tiny, always-available Gauntlet demo project. Its whole job is to give you
something you can start a **real** run against in under a minute.

> **Status (2026-09-15): the loop is closed end to end.** Each unit
> builds in a workspace seeded from the project plus earlier settled
> work (no more rebuilding from nothing), the controller promotes
> settled work into the run's integration tree, and `just demo-deliver`
> (`gauntlet deliver --apply`) writes it into the working copy —
> refusing outright if the project drifted underneath the run. Payment
> is measured for real (tokens, wall time, cost against the ceilings),
> admission failures retry with backoff instead of crashing the run,
> and a builder write outside its workspace pins `containment_breach`
> and blocks that attempt's settlement. Live proof run `XHK2W8NSEQ9X8BYVEBET7915C5`
> (archived): 6 dispatches all admitted, AK-9011 and AK-9012 settled
> with durable PASS records, 361,871 tokens / $1.83 measured,
> `deliver --apply` put the settled `calculator.html` in the target
> (digest matches the promoted bytes) and `scripts/check.sh` passes on
> it. AK-9013 stands provisional: its builder wrote a schema-invalid
> result document twice (string `artifacts`, plus a stale invocation
> id on the retry) and the loop fail-closed and escalated honestly —
> see `.agents/CURRENT_STATUS.yml` for the residual and options.

## What this is

- A minimal Gauntlet project (`GAUNTLET.md`, `.wayfinder/`, `.gauntlet/`,
  `.agents/roles/`, `scripts/check.sh`) whose goal is: build
  `calculator.html`, a single-page HTML calculator, with a real check
  command that verifies it actually works.
- `runners.default: prime-agent` — **this dispatches a real agent and
  spends real model tokens.** It is not the mock/replay runner. Budgets in
  `GAUNTLET.md` (`max_rounds: 8`, `max_wall_seconds: 2700`) fit three units
  plus a retry; every run against this project costs something real.
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
just demo-deliver  # writes the settled run's calculator.html into targets/demo-calculator
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
