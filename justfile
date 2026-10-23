# Repository command surface (spec §24.2, plan "Global verification gates").
#
# Bootstrapped incrementally: Task 1 (this file's first version) wires only `ruler`,
# because the ruler must be checkable from the very first task onward. Task 2 adds
# check/test/lint/deny/boundaries/ci/doctor; later tasks add schemas/roles-verify.
# Do not remove `ruler` or change what it calls when extending this file.

# Verify the frozen answer key and reference manifest are unchanged (spec §9.4, I1).
# Run this after every task. If it ever fails, stop and diagnose before continuing —
# see AGENTS.md and scripts/verify-ruler.sh.
ruler:
    ./scripts/verify-ruler.sh

# Type-check every crate and every target (global gate, plan Task 2).
check:
    cargo check --workspace --all-targets

# Run the full test suite. cargo-nextest is the repository runner
# (AGENTS.md "The testing loop"). Includes surfaces-feature tests so the
# repository closure gate observes the workbench HTTP/JSON read path
# (plan T00, gate_binding_rule).
test: db-start
    cargo nextest run --workspace
    cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass --test cli_surface

# Run only the surfaces-feature tests in gauntlet-cli.
test-surfaces: db-start
    cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass --test cli_surface

# Lint gate: clippy warnings are errors, then formatting must match exactly.
# A warning here is a defect, not a suggestion.
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check

# Dependency policy gate (AGENTS.md "Dependencies"). Posture lives in deny.toml;
# a licence, advisory, or source a crate pulls in is a decision, not an accident.
deny:
    cargo deny check

# Architecture boundary tests (spec §24.3): dependency direction, source
# denylist, hardcoded-literal scan. Run it before review, not after — it must
# fail the build before the layering breaks for real.
boundaries:
    cargo nextest run -p gauntlet-arch-tests

# Every global gate in order, ruler last (I1): nothing above may claim green
# while the frozen reference has moved.
ci: db-start check test lint deny boundaries ruler
    @echo "ci: all gates green"

# doctor — the spec §8.5 preflight checks (plan Task 5). Machine report at
# $GAUNTLET_REPORT_DIR/doctor.json plus a human summary; non-zero exit on any
# required failure, so a gate can depend on it.
doctor:
    cargo run -q -p gauntlet-cli -- doctor

# schemas — regenerate docs/schemas/*.json from the typed Annex B definitions.
# Lands with the schema task; must fail loudly until then.
schemas:
    @echo "schemas: not implemented — lands with a later task (spec Annex B schema export)" >&2; exit 2

# harness-matrix — REPORT-ONLY generator of the harness verification matrix (catalog x evidence
# store x PATH lookup). Prints to stdout; `just harness-matrix <path>` writes the file. Runs no harness.
harness-matrix path="":
    cargo run -q -p gauntlet-adapter-harness-catalog --bin harness-matrix -- {{if path == "" { "" } else { "--write " + path }}}

# sync-harness-catalog — REPORT-ONLY drift check of the built-in harness catalog against a
# Herdr donor checkout (default .tmp/donor/herdr). Prints donor HEAD vs pinned commit, re-verifies
# every provenance ref, and lists donor agents/manifests missing from the catalog. Exits 1 on drift.
# Never writes a definition file and never touches the network.
sync-harness-catalog donor=".tmp/donor/herdr":
    cargo run -q -p gauntlet-adapter-harness-catalog --bin sync-harness-catalog -- {{donor}}

# roles-verify — re-read installed harness role state and report drift
# (spec 10.1, plan Task 17): out-of-band edits, source edits since install,
# missing or extra artifacts, or a hand-edited/re-forged roles.lock are a
# typed failure and a non-zero exit.
roles-verify:
    cargo run -q -p gauntlet-cli -- roles verify

# prereg-check — research instrumentation ONLY (`.agents/research/preregistrations/`).
# Verifies a task's frozen preregistration is byte-identical to its recorded SHA-256.
# Deliberately NOT wired into `ruler` or `ci`: the ruler and this instrument are
# independent, and this recipe has no bearing on gates, acceptance, or closure.
prereg-check task:
    ./scripts/prereg-check.sh {{task}}

# Standard Rust developer recipe interface. These focused commands supplement the
# existing global gates; `verify` deliberately delegates to `ci` rather than
# redefining its ordered checks, tests, lint, dependency, boundary, and ruler gates.
check-package package:
    cargo check -p {{package}} --all-targets

test-package package: db-start
    cargo nextest run -p {{package}}

test-doc: db-start
    cargo test --workspace --doc

fmt-check:
    cargo fmt --all -- --check

timings:
    cargo build --workspace --timings

coverage: db-start
    cargo llvm-cov nextest --workspace --html

mutation: db-start
    cargo mutants

cache-stats:
    sccache --show-stats

# --- Gauntlet TUI workbench (Bun + TypeScript + OpenTUI) --------------------
# .agents/plans/gauntlet-tui-prime-workbench.plan.yaml T00, verification.
# workbench_package_root / workbench_toolchain / workbench_gate_composition.
# `workbench/` is a sibling of `crates/`, never nested inside a Cargo crate —
# crates/gauntlet-arch-tests/src/walk.rs's core_tree walks every file under
# the four core crates with no extension filter and treats a symlink or
# oversized file (node_modules, once bun installs) as a hard scan violation,
# not a skip. These are the TypeScript-side counterparts to check/test/lint;
# `just verify` stays the unchanged Rust closure gate and does NOT run them —
# call them explicitly alongside it wherever a task's `gate:` lists both.

# TypeScript strict type-check (tsc --noEmit), the tui-check counterpart to `check`.
tui-check:
    cd workbench && bun run check

# Bun test suite. Omitted selector runs everything currently implemented;
# `all` is the full normative conformance run (plan T25): it mechanically
# enumerates every registered normative suite — each `just tui-test <selector>`
# gate entry of plan task blocks T00A..T24, resolved by
# workbench/src/plan-traceability.ts — runs each one, prints a per-suite table,
# and fails if any normative suite fails, is missing, or selects zero tests.
# Any other value is forwarded to `bun test` as a file/name filter.
tui-test selector='':
    #!/usr/bin/env bash
    set -euo pipefail
    cd workbench
    if [ "{{selector}}" = "all" ]; then
        bun run src/tui-test-all.ts
        exit $?
    fi
    bun test {{selector}}

# Biome lint + format-check (`biome check`), the tui-lint counterpart to `lint`
# and `cargo fmt --check` combined — a formatting-only diff fails this gate.
tui-lint:
    cd workbench && bun run lint

# Document-integrity (bootstrap) or runtime (conformance) preflight —
# scripts/tui-preflight.sh. Missing mode is a usage error (just itself
# refuses a missing required parameter); conformance is NOT_IMPLEMENTED
# until the runtime substrate it would qualify (Prime host, projection
# store, OpenTUI shell, target terminal matrix) exists.
tui-preflight mode:
    ./scripts/tui-preflight.sh {{mode}}

# TUI benchmark runner — one selector per frozen benchmark suite
# (bun-substrate T03, sqlite-replay T06, terminal-path T13, focus-latency and
# projection-worker T17, large-case and long-horizon T21); `all-required`
# includes every required benchmark, per the plan's recipe contract.
tui-bench selector:
    #!/usr/bin/env bash
    set -euo pipefail
    cd workbench
    if [ "{{selector}}" = "bun-substrate" ]; then
        bun run src/bun-substrate.bench.ts
    elif [ "{{selector}}" = "focus-latency" ]; then
        bun run src/focus-latency.bench.ts
    elif [ "{{selector}}" = "projection-worker" ]; then
        bun run src/projection-worker.bench.ts
    elif [ "{{selector}}" = "sqlite-replay" ]; then
        bun run src/sqlite-replay.bench.ts
    elif [ "{{selector}}" = "terminal-path" ]; then
        bun run src/terminal-path.bench.ts
    elif [ "{{selector}}" = "large-case" ]; then
        bun run src/large-case.bench.ts
    elif [ "{{selector}}" = "long-horizon" ]; then
        bun run src/long-horizon.bench.ts
    elif [ "{{selector}}" = "all-required" ]; then
        for bench in bun-substrate sqlite-replay focus-latency projection-worker terminal-path large-case long-horizon; do
            echo "=== tui-bench all-required: $bench ==="
            bun run "src/$bench.bench.ts" || exit 1
        done
        echo "tui-bench all-required: PASS — all 7 required benchmarks exited 0"
    else
        echo "tui-bench {{selector}}: NOT_IMPLEMENTED — unknown benchmark selector (registered: bun-substrate, focus-latency, projection-worker, sqlite-replay, terminal-path, large-case, long-horizon, all-required)" >&2
        exit 2
    fi

# Release/package target (plan T20): builds the pinned-target standalone
# artifact (bun-linux-x64 --compile), LAUNCHES it in a real PTY at 120x40 and
# qualifies the rendered shell, then writes workbench/dist/release-manifest.json
# (artifact sha256, bun version, entry, lockfile digest, dependency list,
# launch qualification, support facts). Exit 0 requires artifact + manifest +
# clean launch. Other modes are NOT_IMPLEMENTED until a task declares them.
tui-build mode:
    #!/usr/bin/env bash
    set -euo pipefail
    cd workbench
    if [ "{{mode}}" = "release" ]; then
        bun run src/release-manifest.ts
    else
        echo "tui-build {{mode}}: NOT_IMPLEMENTED — no other package target is declared yet (plan T20 owns 'release')" >&2
        exit 2
    fi

# Bring up everything needed to use the workbench interactively: builds and
# starts the core canonical surface (`gauntlet serve`, --features surfaces)
# in the background on a loopback bind, waits for it to answer, installs
# workbench dependencies if missing, auto-detects a local `prime-agent`
# binary so Prime delegation is live when one is installed (D6: skipped
# harmlessly, never degrading Gauntlet, when absent), then launches the
# workbench TUI in THIS terminal so you can start prompting immediately.
#
# `gauntlet serve` refuses to open until a real run has been composed
# somewhere (durable state must exist — nothing is fabricated).
#
# With NO `project` argument (the default, unchanged from before): this
# repo's OWN root GAUNTLET.md is its real self-hosting configuration, not
# a demo target, so this recipe never composes a run there. Instead, on
# first use it seeds an isolated scratch project at
# workbench/.dev/scratch-project (a copy of this repo's own
# tests/fixtures/target E2E fixture) and composes one zero-cost run there
# with the deterministic mock runner — entirely separate from this repo's
# real config, safe to delete any time. The backend then serves from that
# scratch project.
#
# With a project path (`just tui-dev-up <path>`; plan increment D): <path>
# must already be a real Gauntlet project (a GAUNTLET.md at its root) — it
# is served AS-IS, never copied, never seeded, and NEVER given a
# mock-composed run (a real project must never carry one — that discipline
# is load-bearing, not a style choice). <path> is resolved as given
# (absolute, or relative to the current directory) if it exists; otherwise
# `targets/<path>` (repo-root-relative) is tried, so a bare name like
# `just tui-dev-up demo-calculator` finds `targets/demo-calculator` without
# spelling out the full path. `targets/` is an operator convenience folder
# for target-project working copies (this recipe's default lookup
# location; `just demo-up` materializes `targets/demo-calculator` into
# it) — never committed. Since `gauntlet serve` won't open until a run
# exists, and composing one
# may mean dispatching a real paid agent, this recipe does not wait for
# that: it launches a pidfile-tracked background supervisor
# (`scripts/serve-supervisor.sh`) that keeps retrying `gauntlet serve`
# until an attempt actually stays up (i.e. until the operator — or the
# workbench's own `run.start`, once bound — composes the first run), and
# launches the workbench TUI right away so you can start that first run
# from inside it. `GAUNTLET_WORKBENCH_PROJECT_ROOT` is exported so the
# workbench's `run.start` binding dispatches against the right project —
# config defaults now auto-discover `project_root` by walking up from the
# launch directory, but this recipe launches the workbench from inside
# `workbench/`, which would otherwise auto-discover THIS repo's own root
# GAUNTLET.md, so the override stays required. `gauntlet_binary` auto-
# discovery (PATH `gauntlet`, else the checkout's `target/debug/gauntlet`)
# already matches what this recipe would set, so it is left unset.
#
# Quit the TUI with `q` any time — the backend (and, in project mode, the
# supervisor) keeps running in the background; stop it with
# `just tui-dev-down` (pass the same `<path>` to target a
# project-mode instance). Safe to re-run: reuses an already-answering
# backend instead of starting a second one, and (default mode) an
# already-seeded scratch project instead of reseeding.
tui-dev-up project='':
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    DEV_DIR="$ROOT/workbench/.dev"
    BIND="${GAUNTLET_API_BIND:-127.0.0.1:4000}"
    GAUNTLET_BIN="$ROOT/target/debug/gauntlet"
    mkdir -p "$DEV_DIR"

    for tool in cargo bun curl; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "tui-dev-up: '$tool' is not on PATH — install it first (see docs/how-to/operate-the-tui.md, Prerequisites)." >&2
            exit 1
        fi
    done

    probe() { curl -fsS -o /dev/null --max-time 1 "http://$BIND/api/v1/capabilities" 2>/dev/null; }

    PROJECT_ARG="{{project}}"

    if [ -z "$PROJECT_ARG" ]; then
        # --- Default mode: unchanged mock scratch-project behavior. -----
        SCRATCH="$DEV_DIR/scratch-project"
        PIDFILE="$DEV_DIR/gauntlet-serve.pid"
        LOGFILE="$DEV_DIR/gauntlet-serve.log"

        if probe; then
            echo "tui-dev-up: something is already answering at http://$BIND — reusing it."
        else
            if [ -f "$PIDFILE" ] && ! kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
                echo "tui-dev-up: clearing a stale pidfile from a previous run."
                rm -f "$PIDFILE"
            fi

            echo "tui-dev-up: building the core surface (cargo build -p gauntlet-cli --features surfaces) — first run can take a minute..."
            cargo build -p gauntlet-cli --features surfaces

            if [ ! -f "$SCRATCH/GAUNTLET.md" ]; then
                echo "tui-dev-up: seeding an isolated local dev project at workbench/.dev/scratch-project"
                echo "tui-dev-up: (a copy of this repo's own E2E fixture target — not this repo's real config; safe to delete any time)."
                rm -rf "$SCRATCH"
                cp -r "$ROOT/tests/fixtures/target" "$SCRATCH"
                chmod +x "$SCRATCH/scripts/check.sh"
            fi

            compose_scratch_run() {
                echo "tui-dev-up: composing a first run in the scratch project (deterministic mock runner — zero cost, zero real dispatch)..."
                (cd "$SCRATCH" && \
                    GAUNTLET_MOCK_SCENARIO=e2e-happy \
                    GAUNTLET_TOOL_TEST=./scripts/check.sh \
                    GAUNTLET_CLOCK=deterministic \
                    GAUNTLET_STALL_TIMEOUT_MS=60000 \
                    GAUNTLET_RETRY_BASE_MS=100 \
                    GAUNTLET_RETRY_MAX_MS=2000 \
                    GAUNTLET_MAX_ATTEMPTS_PER_UNIT=3 \
                    "$GAUNTLET_BIN" run "workbench dev bootstrap: seed a run so the surface has state to serve" \
                    >"$DEV_DIR/scratch-run.log" 2>&1) || true
            }

            start_serve() {
                echo "tui-dev-up: starting 'gauntlet serve' on $BIND (project: workbench/.dev/scratch-project, log: workbench/.dev/gauntlet-serve.log)..."
                # A single simple command backgrounded on its own line (not a
                # `cd X && CMD &` compound list) so `$!` is unambiguously the
                # 'gauntlet serve' pid itself, not a wrapper subshell's pid.
                (
                    cd "$SCRATCH"
                    GAUNTLET_API_BIND="$BIND" nohup "$GAUNTLET_BIN" serve >"$LOGFILE" 2>&1 &
                    echo $! > "$PIDFILE"
                )
            }

            attempt_ready() {
                ready=0
                for _ in $(seq 1 "$1"); do
                    if probe; then
                        ready=1
                        return
                    fi
                    if [ -f "$PIDFILE" ] && ! kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
                        return
                    fi
                    sleep 0.25
                done
            }

            start_serve
            attempt_ready 20

            if [ "$ready" -ne 1 ] && grep -q "no run has been composed here yet" "$LOGFILE" 2>/dev/null; then
                compose_scratch_run
                rm -f "$PIDFILE"
                start_serve
                attempt_ready 20
            fi

            if [ "$ready" -ne 1 ]; then
                echo "tui-dev-up: 'gauntlet serve' did not answer at http://$BIND. Last log lines:" >&2
                tail -n 20 "$LOGFILE" >&2 || true
                rm -f "$PIDFILE"
                exit 1
            fi
            echo "tui-dev-up: core surface is up at http://$BIND"
        fi
        export GAUNTLET_WORKBENCH_PROJECT_ROOT="$SCRATCH"
    else
        # --- Project mode (plan increment D): a real project, served ----
        # --- as-is; a mock run is NEVER composed here. -------------------
        if [ -d "$PROJECT_ARG" ]; then
            PROJECT="$(cd "$PROJECT_ARG" && pwd)"
        elif [ -d "$ROOT/targets/$PROJECT_ARG" ]; then
            PROJECT="$(cd "$ROOT/targets/$PROJECT_ARG" && pwd)"
        else
            echo "tui-dev-up: no such directory '$PROJECT_ARG' (also tried targets/$PROJECT_ARG)." >&2
            exit 1
        fi
        if [ ! -f "$PROJECT/GAUNTLET.md" ]; then
            echo "tui-dev-up: $PROJECT has no GAUNTLET.md — the project path must name a real Gauntlet project root." >&2
            exit 1
        fi

        SLUG="$(printf '%s' "$(basename "$PROJECT")" | tr -c 'A-Za-z0-9_-' '-')"
        PIDFILE="$DEV_DIR/gauntlet-serve-$SLUG.pid"
        LOGFILE="$DEV_DIR/gauntlet-serve-$SLUG.log"
        SUPERVISOR_PIDFILE="$DEV_DIR/gauntlet-serve-$SLUG.supervisor.pid"
        STOPFILE="$DEV_DIR/gauntlet-serve-$SLUG.stop"
        rm -f "$STOPFILE"

        echo "tui-dev-up: building the core surface (cargo build -p gauntlet-cli --features surfaces) if needed..."
        if [ ! -x "$GAUNTLET_BIN" ]; then
            cargo build -p gauntlet-cli --features surfaces
        fi

        if probe; then
            echo "tui-dev-up: something is already answering at http://$BIND — reusing it."
        elif [ -f "$SUPERVISOR_PIDFILE" ] && kill -0 "$(cat "$SUPERVISOR_PIDFILE")" 2>/dev/null; then
            echo "tui-dev-up: a serve-supervisor for $PROJECT is already retrying in the background (pid $(cat "$SUPERVISOR_PIDFILE"))."
        else
            echo "tui-dev-up: no run exists yet in $PROJECT — 'gauntlet serve' will refuse to open until one does."
            echo "tui-dev-up: starting a background supervisor that retries 'gauntlet serve' every few seconds (log: $LOGFILE)."
            echo "tui-dev-up: start the first run from inside the workbench (or run 'gauntlet run \"<intent>\"' in $PROJECT) — the supervisor picks it up as soon as one exists."
            nohup "$ROOT/scripts/serve-supervisor.sh" "$GAUNTLET_BIN" "$PROJECT" "$BIND" "$PIDFILE" "$LOGFILE" "$STOPFILE" >>"$LOGFILE" 2>&1 &
            echo $! >"$SUPERVISOR_PIDFILE"
        fi
        export GAUNTLET_WORKBENCH_PROJECT_ROOT="$PROJECT"
    fi

    if [ ! -d workbench/node_modules ]; then
        echo "tui-dev-up: installing workbench dependencies (first run only)..."
        (cd workbench && bun install)
    fi

    if command -v prime-agent >/dev/null 2>&1; then
        PRIME_BIN="$(command -v prime-agent)"
        echo "tui-dev-up: found prime-agent at $PRIME_BIN — Prime delegation will be live."
        export GAUNTLET_WORKBENCH_PRIME_BINARY="$PRIME_BIN"
    else
        echo "tui-dev-up: no 'prime-agent' on PATH — Gauntlet inspection works fully; Prime delegation stays unavailable until one is installed (D6: no degradation)."
    fi

    # The prime-agent runner's `stop`/`list` are daemon-backed (a Unix
    # socket at /tmp/prime-agent-$UID/daemon.sock); nothing in prime-agent's
    # own CLI starts that daemon implicitly, and without it a stall-timeout
    # cancel fails fatally. Start the daemon ONLY when this project actually
    # uses runners.default: prime-agent, and ONLY when no daemon is already
    # up — scripts/prime-agent-daemon.sh never touches a pre-existing
    # daemon (an operator's own, or another session's) and records whether
    # it was the one that started it, so `tui-dev-down` stops it only then.
    if [ -f "$GAUNTLET_WORKBENCH_PROJECT_ROOT/GAUNTLET.md" ] \
       && grep -qE '^\s*default:\s*prime-agent\s*$' "$GAUNTLET_WORKBENCH_PROJECT_ROOT/GAUNTLET.md"; then
        "$ROOT/scripts/prime-agent-daemon.sh" ensure "$DEV_DIR"
    fi

    echo "tui-dev-up: launching the workbench — press 'q' to quit (the backend keeps running; 'just tui-dev-down' stops it)."
    export GAUNTLET_ENDPOINT="http://$BIND"
    cd workbench
    exec bun run src/main.ts

# Tear down what `just tui-dev-up` started: stops the tracked 'gauntlet
# serve' background process (and, in project mode, its serve-supervisor),
# if any. Pass the SAME `<path>` you gave `tui-dev-up` to target a
# project-mode instance; omit it to target the default mock scratch
# instance. Safe to run any time, including when nothing is running — it
# only ever touches processes it started itself (tracked by pidfile),
# never guesses at what else might be on the port.
tui-dev-down project='':
    #!/usr/bin/env bash
    set -euo pipefail
    DEV_DIR="workbench/.dev"
    PROJECT_ARG="{{project}}"

    stop_pidfile() {
        local label="$1" pidfile="$2"
        if [ ! -f "$pidfile" ]; then
            echo "tui-dev-down: nothing to stop for $label (no tracked pid)."
            return 0
        fi
        local pid
        pid="$(cat "$pidfile")"
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "tui-dev-down: $label (pid $pid) was already stopped."
            rm -f "$pidfile"
            return 0
        fi
        echo "tui-dev-down: stopping $label (pid $pid)..."
        kill "$pid" 2>/dev/null || true
        for _ in $(seq 1 20); do
            if ! kill -0 "$pid" 2>/dev/null; then
                break
            fi
            sleep 0.25
        done
        if kill -0 "$pid" 2>/dev/null; then
            echo "tui-dev-down: $label still running after 5s — sending SIGKILL."
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$pidfile"
        echo "tui-dev-down: $label stopped."
    }

    if [ -z "$PROJECT_ARG" ]; then
        stop_pidfile "'gauntlet serve' (default scratch project)" "$DEV_DIR/gauntlet-serve.pid"
    else
        PROJECT="$(cd "$PROJECT_ARG" 2>/dev/null && pwd || echo "$PROJECT_ARG")"
        SLUG="$(printf '%s' "$(basename "$PROJECT")" | tr -c 'A-Za-z0-9_-' '-')"
        STOPFILE="$DEV_DIR/gauntlet-serve-$SLUG.stop"
        touch "$STOPFILE"
        stop_pidfile "serve-supervisor for $PROJECT" "$DEV_DIR/gauntlet-serve-$SLUG.supervisor.pid"
        stop_pidfile "'gauntlet serve' for $PROJECT" "$DEV_DIR/gauntlet-serve-$SLUG.pid"
        rm -f "$STOPFILE"
    fi

    # Only stops the prime-agent daemon if THIS recipe set was the one that
    # started it (scripts/prime-agent-daemon.sh checks its own
    # started-by-us marker) — an operator's own daemon, or one from an
    # unrelated session, is left running.
    ./scripts/prime-agent-daemon.sh stop "$DEV_DIR"

# Bring up the always-available demo-calculator project (operator ask:
# "set up a tiny demo run that is always available"): materializes
# examples/demo-calculator into a fresh working copy at
# targets/demo-calculator (examples/ itself is never touched or
# mutated), then delegates to `tui-dev-up <targets copy>` — same
# build-if-missing, same pidfile-tracked serve-supervisor, same workbench
# launch — on its OWN port (4100 by default) so it never collides with a
# plain `tui-dev-up` (4000). `runners.default` in the template is the
# REAL `prime-agent` runner: starting a run against this project from
# inside the workbench (or via `gauntlet run "<intent>"` in the targets
# copy) dispatches a real agent and spends real tokens. Safe to re-run:
# reuses an already-materialized copy instead of overwriting it, so an
# in-progress run's workspace state is preserved.
demo-up:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    TARGET="$ROOT/targets/demo-calculator"
    BIND="${GAUNTLET_API_BIND:-127.0.0.1:4100}"

    mkdir -p "$(dirname "$TARGET")"
    if [ ! -f "$TARGET/GAUNTLET.md" ]; then
        echo "demo-up: materializing examples/demo-calculator into targets/demo-calculator (a working copy; examples/ is never modified)."
        rm -rf "$TARGET"
        cp -r "$ROOT/examples/demo-calculator" "$TARGET"
        chmod +x "$TARGET/scripts/check.sh" "$TARGET/scripts/eval-calculator.js"
    else
        echo "demo-up: reusing the existing working copy at targets/demo-calculator (delete it, or run 'just demo-down', to start over)."
    fi

    echo "demo-up: this project's runners.default is prime-agent — a REAL run against it dispatches a real agent and spends real tokens."
    # Dev env layers (.env.defaults, then your gitignored .env): the REQUIRED,
    # env-only control-loop settings live there. Without them the dry-run
    # passes but a real run refuses at once, so a run started from the TUI
    # would die immediately and the demo would not be "always available".
    . "$ROOT/scripts/load-env.sh"
    # This project's real check command is the `test` facet's instrument.
    # ABSOLUTE on purpose: the facet runs it with cwd = the unit's workspace
    # (where the builder's work lives), and a relative path would be looked
    # up there, where no scripts/ exists.
    export GAUNTLET_TOOL_TEST="${GAUNTLET_TOOL_TEST:-$TARGET/scripts/check.sh}"
    # Run state OUT of /tmp (a WSL restart wipes it, and with it the run's
    # state, workspaces and evidence). Absolute on purpose: the workspace
    # provider refuses any path containing a `..` component. serve, the TUI
    # and any run it starts all inherit these, so all three agree.
    export GAUNTLET_STATE_DIR="${GAUNTLET_STATE_DIR:-$DEV_RUNS_ROOT/demo-calculator/state}"
    export GAUNTLET_ARTIFACT_DIR="${GAUNTLET_ARTIFACT_DIR:-$DEV_RUNS_ROOT/demo-calculator/evidence}"
    mkdir -p "$GAUNTLET_STATE_DIR" "$GAUNTLET_ARTIFACT_DIR"

    echo "demo-up: bringing up serve + workbench on http://$BIND ..."
    GAUNTLET_API_BIND="$BIND" "{{just_executable()}}" --justfile "{{justfile()}}" tui-dev-up "$TARGET"

# Tear down the demo-calculator run: stops exactly what `just demo-up`
# started (delegates to `tui-dev-down <targets copy>`) and then deletes
# ONLY targets/demo-calculator — never the whole targets/ folder (the
# operator keeps their own projects there) and never
# examples/demo-calculator. Idempotent: safe to run twice, and prints a
# clear message when nothing is running or nothing to delete.
demo-down:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    TARGET="$ROOT/targets/demo-calculator"

    "{{just_executable()}}" --justfile "{{justfile()}}" tui-dev-down "$TARGET"

    if [ -d "$TARGET" ]; then
        rm -rf "$TARGET"
        echo "demo-down: removed targets/demo-calculator (targets/ itself is left in place)."
    else
        echo "demo-down: no working copy to remove (nothing to do)."
    fi

# Delivers the demo-calculator run's settled work product into the
# working copy: `gauntlet deliver --apply` under the SAME env `demo-up`
# runs against (GAUNTLET_STATE_DIR / GAUNTLET_ARTIFACT_DIR under
# DEV_RUNS_ROOT) — no new locations invented. Requires a working copy at
# targets/demo-calculator (`just demo-up` first) and a settled run.
demo-deliver:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    TARGET="$ROOT/targets/demo-calculator"
    if [ ! -f "$TARGET/GAUNTLET.md" ]; then
        echo "demo-deliver: no working copy at targets/demo-calculator — run 'just demo-up' first."
        exit 1
    fi
    . "$ROOT/scripts/load-env.sh"
    export GAUNTLET_STATE_DIR="${GAUNTLET_STATE_DIR:-$DEV_RUNS_ROOT/demo-calculator/state}"
    export GAUNTLET_ARTIFACT_DIR="${GAUNTLET_ARTIFACT_DIR:-$DEV_RUNS_ROOT/demo-calculator/evidence}"
    GAUNTLET_BIN="$ROOT/target/debug/gauntlet"
    if [ ! -x "$GAUNTLET_BIN" ]; then
        cargo build -p gauntlet-cli --bin gauntlet
    fi
    cd "$TARGET"
    "$GAUNTLET_BIN" deliver --apply

# --- Local PostgreSQL substrate (durable state store) ----------------------
# PostgreSQL is the canonical state store; SQLite is removed. The local
# substrate is a pg0-managed PostgreSQL 18 instance named `gauntlet-dev` on
# 127.0.0.1:54329. pg0 is pinned by version and sha256 under .tmp/bin/ (never
# committed, never fetched via curl|bash). GAUNTLET_DATABASE_URL overrides the
# connection URI; the adapter's built-in default is this instance.

# Download the pinned pg0 release binary into .tmp/bin/pg0 (idempotent). Refuses
# to install a binary whose sha256 does not match the pin.
db-install:
    #!/usr/bin/env bash
    set -euo pipefail
    PG0_VERSION=v0.15.2
    PG0_ASSET=pg0-linux-x86_64-gnu
    PG0_SHA256=06c4678ff7b461dba8ed1f95c5e202ead3b57b70f549a22c17a94985121c0bf5
    BIN=.tmp/bin/pg0
    if [ -x "$BIN" ] && [ "$(sha256sum "$BIN" | cut -d' ' -f1)" = "$PG0_SHA256" ]; then
        echo "db-install: pg0 $PG0_VERSION already installed at $BIN (sha256 verified)."
        exit 0
    fi
    mkdir -p .tmp/pg0-dl .tmp/bin
    gh release download "$PG0_VERSION" --repo vectorize-io/pg0 -p "$PG0_ASSET" -D .tmp/pg0-dl --clobber
    GOT="$(sha256sum ".tmp/pg0-dl/$PG0_ASSET" | cut -d' ' -f1)"
    if [ "$GOT" != "$PG0_SHA256" ]; then
        echo "db-install: sha256 mismatch for $PG0_ASSET ($PG0_VERSION): got $GOT, expected $PG0_SHA256 — refusing to install." >&2
        exit 1
    fi
    install -m 755 ".tmp/pg0-dl/$PG0_ASSET" "$BIN"
    echo "db-install: pg0 $PG0_VERSION installed at $BIN (sha256 verified)."

# Start the gauntlet-dev PostgreSQL instance on port 54329 (idempotent: a
# running instance is left untouched).
db-start: db-install
    #!/usr/bin/env bash
    set -euo pipefail
    BIN=.tmp/bin/pg0
    if "$BIN" info --name gauntlet-dev 2>/dev/null | grep -q "is running"; then
        echo "db-start: gauntlet-dev already running on port 54329."
        exit 0
    fi
    "$BIN" start --name gauntlet-dev --port 54329

# Stop the gauntlet-dev instance (no-op when it is not running or not installed).
db-stop:
    #!/usr/bin/env bash
    set -euo pipefail
    BIN=.tmp/bin/pg0
    if [ ! -x "$BIN" ]; then
        echo "db-stop: pg0 is not installed — nothing to stop."
        exit 0
    fi
    if "$BIN" info --name gauntlet-dev 2>/dev/null | grep -q "is running"; then
        "$BIN" stop --name gauntlet-dev
    else
        echo "db-stop: gauntlet-dev is not running."
    fi

# Show the gauntlet-dev instance status (stopped, running, or missing).
db-status:
    #!/usr/bin/env bash
    set -euo pipefail
    BIN=.tmp/bin/pg0
    if [ ! -x "$BIN" ]; then
        echo "db-status: pg0 is not installed — run 'just db-install'."
        exit 0
    fi
    "$BIN" info --name gauntlet-dev

# Print the gauntlet-dev PostgreSQL connection URI (the GAUNTLET_DATABASE_URL value).
db-url:
    #!/usr/bin/env bash
    set -euo pipefail
    BIN=.tmp/bin/pg0
    URI="$("$BIN" info --name gauntlet-dev 2>/dev/null | sed -n 's/^URI: //p')"
    if [ -z "$URI" ]; then
        echo "db-url: gauntlet-dev has no connection URI — run 'just db-start' first." >&2
        exit 1
    fi
    echo "$URI"

# Drop the gauntlet-dev instance (stops it and deletes all data), then start a
# fresh one.
db-reset: db-install
    #!/usr/bin/env bash
    set -euo pipefail
    BIN=.tmp/bin/pg0
    if "$BIN" info --name gauntlet-dev 2>/dev/null | grep -qE "is (running|stopped)"; then
        "$BIN" drop --name gauntlet-dev --force
    else
        echo "db-reset: gauntlet-dev does not exist — nothing to drop."
    fi
    "{{just_executable()}}" --justfile "{{justfile()}}" db-start

verify: db-start ci
