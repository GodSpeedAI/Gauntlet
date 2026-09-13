# How-To: Operate the Gauntlet Workbench TUI

This guide explains how to launch, navigate, operate, and troubleshoot the Gauntlet Operator Workbench terminal user interface.

---

## Goal

Start an interactive terminal session to monitor live execution, navigate cases and evidence, hoist context horizons, adjudicate human escalations, and issue authorized commands.

---

## Prerequisites

- **Bun Runtime**: Bun $\ge 1.4.0$ installed and available in `$PATH`.
- **Gauntlet Core**: The `gauntlet` CLI compiled with the `surfaces` feature enabled (`cargo build -p gauntlet-cli --features surfaces`).
- **Terminal Capabilities**: A terminal emulator with dimensions of at least $80\times 24$ (recommended: $120\times 40$ or wider).

---

## Launching the Workbench

### Option A: Development Mode (Bun)

To launch directly from TypeScript source during development:

```bash
cd workbench
bun run src/main.ts
```

Or via repository `just` recipe:

```bash
just tui-check && cd workbench && bun run src/main.ts
```

### Option B: Standalone Release Binary

To build the self-contained release executable compiled via Bun:

```bash
just tui-build release
```

This compiles `workbench/dist/gauntlet-workbench` and validates the release manifest (`workbench/dist/release-manifest.json`). Run the resulting binary directly:

```bash
./workbench/dist/gauntlet-workbench
```

### Quick Start: `just tui-dev-up` / `just tui-dev-down`

The easiest way to get a working, connected workbench: `just tui-dev-up` builds and starts
the core surface in the background (waiting until it actually answers), installs workbench
dependencies if this is a first run, wires up a local `prime-agent` automatically if one is
installed, and then launches the interactive workbench right in your terminal. Quit the TUI
with `q` any time — the backend keeps running; stop it with `just tui-dev-down`. Safe to
run `tui-dev-up` again later: it reuses an already-running backend instead of starting a
second one.

```bash
just tui-dev-up      # build + start the core surface, then launch the TUI
# ... use the workbench; press q to quit back to the shell ...
just tui-dev-down    # stop the background core surface
```

Note: `gauntlet serve` refuses to start until at least one real Gauntlet run has been
composed in this project (it has genuine durable state to open — nothing is fabricated).
If `tui-dev-up` reports that, compose one first with `gauntlet run "<intent>"`, then
re-run `tui-dev-up`.

### Pointing `tui-dev-up` at a real project

`just tui-dev-up <path>` (plan increment D) serves a REAL Gauntlet project instead of
the default mock scratch project. `<path>` must already contain a `GAUNTLET.md` — it is
served as-is, never copied, and a mock run is NEVER composed into it (a real project must
never carry one). `<path>` is resolved as given (absolute, or relative to the current
directory) if it exists; otherwise `targets/<path>` (repo-root-relative) is tried, so a
bare name like `just tui-dev-up demo-calculator` finds `targets/demo-calculator` without
spelling out the full path. `targets/` is an operator convenience folder for
target-project working copies — never committed (see `.gitignore`); only the templates
under `examples/` are. Since `gauntlet serve` refuses to open until a run exists, and
composing one there may mean dispatching a real, paid agent, this mode does not block
waiting for that: it launches a pidfile-tracked background supervisor
(`scripts/serve-supervisor.sh`) that keeps retrying `gauntlet serve` every few seconds,
and brings up the workbench TUI right away so you can start the first run from inside it
(or from another terminal with `gauntlet run "<intent>"` in that project) — the supervisor
picks it up the moment a run exists.

```bash
just tui-dev-up /path/to/real-project    # supervisor + workbench, no mock run ever composed
just tui-dev-up demo-calculator          # short form: resolves targets/demo-calculator
# ... start the first run from inside the workbench, or from another terminal ...
just tui-dev-down /path/to/real-project  # stop exactly that supervisor + serve
```

Omit the path to keep the original default (mock scratch project) behavior; give the
SAME path to `tui-dev-down` you gave `tui-dev-up` so it tears down the right instance.

### The always-available demo-calculator (`just demo-up` / `just demo-down`)

`examples/demo-calculator` is a tiny, always-available Gauntlet project (see its own
`README.md`) whose goal is: dispatch a REAL agent to build a single-page HTML calculator,
then verify the result with a real, deterministic, no-network check command. It uses
`runners.default: prime-agent` — starting a run against it spends real tokens, so its
budget ceilings are kept small (`max_rounds: 3`, `max_wall_seconds: 900`, `max_cost_usd: 2`).

```bash
just demo-up      # materializes targets/demo-calculator, brings up serve + workbench on :4100
# ... inside the workbench (or via `gauntlet run "<intent>"` in targets/demo-calculator),
# ... start a run and watch a real agent attempt the calculator ...
just demo-down    # stops what demo-up started and removes targets/demo-calculator
```

`demo-up` never modifies `examples/demo-calculator` itself — it materializes a fresh
working copy at `targets/demo-calculator` (via `tui-dev-up <path>` above, on its own port
so it never collides with a plain `tui-dev-up`). `demo-down` removes only
`targets/demo-calculator`, never the whole `targets/` folder (the operator may keep other
projects there). `demo-down` is idempotent: safe
to run any time, including when nothing is running.

### Connecting to the Core Loopback Service Manually

The workbench's default endpoint is `http://127.0.0.1:4000` (`workbench/src/config.ts`).
`gauntlet serve` (spec §20.2; requires a build with `--features surfaces`) never guesses an
address — it reads its bind **only** from the `GAUNTLET_API_BIND` environment variable and
takes no `--port` or other flags. Start it in a separate terminal:

```bash
cargo build -p gauntlet-cli --features surfaces
GAUNTLET_API_BIND=127.0.0.1:4000 ./target/debug/gauntlet serve
```

If using a custom endpoint, session token, or actor, configure environment variables
(matching bind and endpoint — the surface is loopback-only, per §20.2):

```bash
export GAUNTLET_API_BIND="127.0.0.1:9000"
export GAUNTLET_ENDPOINT="http://127.0.0.1:9000"
export GAUNTLET_SESSION_TOKEN="my-secret-session-token"
export GAUNTLET_SESSION_ACTOR="operator-lead"
./workbench/dist/gauntlet-workbench
```

---

## Navigation Procedures

### 1. View Switching

The Primary surface displays three core question-oriented views. Switch between them with direct numerical keys:

- Press `1`: **Work View** ("What are we trying to move?"). Displays open escalations, active units, and completed units.
- Press `2`: **Chat View** ("What was communicated?"). Displays operator-agent conversational turns.
- Press `3`: **Trace View** ("What actually happened?"). Displays the immutable, append-only execution log.

### 2. Surface Traversal

To cycle focus between the active layout regions:

- Press `Tab`: Advances focus clockwise through `Navigator` $\rightarrow$ `Primary` $\rightarrow$ `Inspector` $\rightarrow$ `Composer`.
- Press `Esc`: Dismisses auxiliary overlays and returns focus immediately to `Primary`.

### 3. Focus Selection

1. Focus the **Navigator** surface using `Tab`.
2. Use `Up` and `Down` arrow keys to highlight a Case, Run, Unit, or Evidence record.
3. Press `Enter`: Pins the highlighted item as the active `Focus`.
4. Observe the **Inspector** surface on the right: it immediately displays the item's identity, relational graph edges, evidence records, and allowable affordances.

### 4. Hoisting Context Horizons

When working in large repositories with dozens of units, hoist your perceptual horizon to isolate focus:

1. Select a specific Unit (e.g. `AK-014`) in the Navigator or Work view.
2. Press `h`: **Hoist**. The active horizon narrows to `AK-014`. The header breadcrumb updates (`Case › Run 12 › Unit AK-014`), and the Work and Trace views filter out all unrelated events.
3. Press `u`: **Unhoist**. Returns the horizon to the parent scope without mutating or discarding any underlying state.

---

## Executing Commands & Handling Escalations

### Handling Human Escalations and Approvals

When a Unit with `blast_radius: irreversible` or an authority conflict occurs:

1. The header highlights an attention indicator: `! attention 1`.
2. The Work view pins the blocked item to the top of the list with token `readiness.blocked`.
3. If an immediate operator response is required, the workbench initiates a **Composer Takeover**, temporarily displaying the approval prompt:
   ```text
   APPROVAL REQUIRED: Unit AK-003 (Production schema migration)
   Action: Resolve escalation / Proceed with migration
   [Enter] to approve · [Esc] to dismiss
   ```
4. Enter your decision note and press `Enter`. The `OperationGateway` re-validates the gate, transmits the authorized signature to Gauntlet core, commits the settlement, and restores your previous composer draft.

### Using the Command Palette

To execute commands or inspect unavailable actions without leaving the keyboard:

1. Press `p`: Opens the **Command Palette** overlay.
2. Use `Up` and `Down` to browse available views, focusable entities, and executable operations.
3. Actions that are currently blocked display their machine reason codes (e.g. `blocked: authority_mismatch`).
4. Press `Enter` to switch view, select focus, or dispatch the action.
5. Press `Esc` or `p` to close the palette without taking action.

### Using the Composer

1. Press `e`: Moves focus directly into the bottom **Composer** surface.
2. Type your message or instruction.
3. Press `Enter` to submit to the active mode destination (`message`, `operation`, `prime`, `kernel`).

---

## Operational Runbook & Emergency Recovery

### Terminal Corrupted or Output Garbled

If the process crashes abnormally while the host terminal is in raw mode:

1. Press `Ctrl+C` or `q` if responsive.
2. If your shell input is invisible or broken, type the following blindly and hit Enter:
   ```bash
   stty sane && reset
   ```
3. This restores standard terminal line buffering, character echo, and cursor visibility.

### Screen Geometry Alert (`BelowMinimum`)

If you see:
```text
! Gauntlet needs at least 80x24. This terminal is 72x20.
Resize the window, or press q to quit. (admitted work keeps running)
```
- **Fix**: Resize or maximize your terminal window until it reaches at least 80 columns by 24 rows.
- The interface will automatically restore the full layout without losing your current focus or interrupting running jobs.

---

## Source Trail

- [`workbench/src/main.ts`](../../workbench/src/main.ts) — Process launch, raw mode management, terminal restore.
- [`workbench/src/shell.ts`](../../workbench/src/shell.ts) — Keybinding handlers (`handleKey`), surface switching, palette logic.
- [`workbench/src/views-work.ts`](../../workbench/src/views-work.ts) — Work view rendering and escalation ordering.
- [`workbench/src/composer.ts`](../../workbench/src/composer.ts) — Composer input modes and takeover recovery.
- [`workbench/src/operation-gateway.ts`](../../workbench/src/operation-gateway.ts) — Consequential action dispatch and idempotency.
