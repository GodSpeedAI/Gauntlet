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
test:
    cargo nextest run --workspace
    cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass --test cli_surface

# Run only the surfaces-feature tests in gauntlet-cli.
test-surfaces:
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
ci: check test lint deny boundaries ruler
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

test-package package:
    cargo nextest run -p {{package}}

test-doc:
    cargo test --workspace --doc

fmt-check:
    cargo fmt --all -- --check

timings:
    cargo build --workspace --timings

coverage:
    cargo llvm-cov nextest --workspace --html

mutation:
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
# somewhere (durable state must exist — nothing is fabricated). This
# repo's OWN root GAUNTLET.md is its real self-hosting configuration, not
# a demo target, so this recipe never composes a run there. Instead, on
# first use it seeds an isolated scratch project at
# workbench/.dev/scratch-project (a copy of this repo's own
# tests/fixtures/target E2E fixture) and composes one zero-cost run there
# with the deterministic mock runner — entirely separate from this repo's
# real config, safe to delete any time. The backend then serves from that
# scratch project. Quit the TUI with `q` any time — the backend keeps
# running in the background; stop it with `just tui-dev-down`. Safe to
# re-run: reuses an already-answering backend instead of starting a
# second one, and an already-seeded scratch project instead of reseeding.
tui-dev-up:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    DEV_DIR="$ROOT/workbench/.dev"
    SCRATCH="$DEV_DIR/scratch-project"
    PIDFILE="$DEV_DIR/gauntlet-serve.pid"
    LOGFILE="$DEV_DIR/gauntlet-serve.log"
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

    echo "tui-dev-up: launching the workbench — press 'q' to quit (the backend keeps running; 'just tui-dev-down' stops it)."
    export GAUNTLET_ENDPOINT="http://$BIND"
    cd workbench
    exec bun run src/main.ts

# Tear down what `just tui-dev-up` started: stops the tracked 'gauntlet
# serve' background process, if any. Safe to run any time, including when
# nothing is running — it only ever touches a process it started itself
# (tracked by pidfile), never guesses at what else might be on the port.
tui-dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    DEV_DIR="workbench/.dev"
    PIDFILE="$DEV_DIR/gauntlet-serve.pid"
    LOGFILE="$DEV_DIR/gauntlet-serve.log"

    if [ ! -f "$PIDFILE" ]; then
        echo "tui-dev-down: nothing to stop (no tracked 'gauntlet serve' process)."
        exit 0
    fi

    PID="$(cat "$PIDFILE")"
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "tui-dev-down: the tracked process (pid $PID) was already stopped."
        rm -f "$PIDFILE"
        exit 0
    fi

    echo "tui-dev-down: stopping 'gauntlet serve' (pid $PID)..."
    kill "$PID" 2>/dev/null || true
    for _ in $(seq 1 20); do
        if ! kill -0 "$PID" 2>/dev/null; then
            break
        fi
        sleep 0.25
    done
    if kill -0 "$PID" 2>/dev/null; then
        echo "tui-dev-down: still running after 5s — sending SIGKILL."
        kill -9 "$PID" 2>/dev/null || true
    fi
    rm -f "$PIDFILE"
    echo "tui-dev-down: stopped. Log kept at $LOGFILE if you want to check it."

verify: ci
