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
# `all` means full normative conformance per the plan's recipe contract,
# which is not yet reachable and reports NOT_IMPLEMENTED rather than a green
# placeholder over a suite that doesn't exist; any other value is forwarded
# to `bun test` as a file/name filter.
tui-test selector='':
    #!/usr/bin/env bash
    set -euo pipefail
    cd workbench
    if [ "{{selector}}" = "all" ]; then
        echo "tui-test all: NOT_IMPLEMENTED — no normative suite is registered yet" >&2
        exit 2
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

# TUI benchmark runner — supports plan T03 bun-substrate benchmark.
tui-bench selector:
    #!/usr/bin/env bash
    set -euo pipefail
    cd workbench
    if [ "{{selector}}" = "bun-substrate" ]; then
        bun run src/bun-substrate.bench.ts
    elif [ "{{selector}}" = "sqlite-replay" ]; then
        bun run src/sqlite-replay.bench.ts
    elif [ "{{selector}}" = "terminal-path" ]; then
        bun run src/terminal-path.bench.ts
    else
        echo "tui-bench {{selector}}: NOT_IMPLEMENTED — no benchmark suite exists yet (plan T17/T21)" >&2
        exit 2
    fi

# Not yet implemented: no release/package target exists yet (plan T20).
tui-build mode:
    @echo "tui-build {{mode}}: NOT_IMPLEMENTED — no release/package target exists yet (plan T20)" >&2; exit 2

verify: ci
