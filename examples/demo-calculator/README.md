# demo-calculator

A tiny, always-available Gauntlet demo project. Its whole job is to give you
something you can start a **real** run against in under a minute.

> **Status (2026-09-14): a real run settles.** Run
> `C7T42J1ECBMK45JTB0VB9CX8T4` against installed `prime-agent` settled all
> three units (AK-9011..AK-9013 implemented-and-proven) in 4 iterations and
> ~10 minutes: AK-9012's first attempt was retried, and every settlement is
> backed by the real `scripts/check.sh` run inside the unit's workspace.
> Known gaps: the settled `calculator.html` lives only in the unit's
> workspace, which is removed once the unit completes — nothing copies the
> work product back into this project, so open the evidence, not the file;
> and the `prime-agent` result reports zero token/cost/wall payment, so the
> token and cost ceilings are not exercised by this runner.

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
