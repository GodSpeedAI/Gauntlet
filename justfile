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
# (AGENTS.md "The testing loop").
test:
    cargo nextest run --workspace

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
# (spec §10.1). Lands in plan Task 17; must fail loudly until then.
roles-verify:
    @echo "roles-verify: not implemented — lands in plan Task 17 (role drift detection)" >&2; exit 2

# prereg-check — research instrumentation ONLY (`.agents/research/preregistrations/`).
# Verifies a task's frozen preregistration is byte-identical to its recorded SHA-256.
# Deliberately NOT wired into `ruler` or `ci`: the ruler and this instrument are
# independent, and this recipe has no bearing on gates, acceptance, or closure.
prereg-check task:
    ./scripts/prereg-check.sh {{task}}
