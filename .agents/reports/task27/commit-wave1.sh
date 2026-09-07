#!/bin/bash
# task27 wave-1 milestone commit — orchestrator prep (run only after sweep-3
# reviewed green). Explicit pathspecs; NEVER .jolli/, .tmp/, CURRENT_STATUS*,
# prompts/, agents/, or the operator's TUI workstream files.
set -euo pipefail
cd /home/sprime01/projects/gauntlet

git add \
  crates/gauntlet-app/src/conformance \
  crates/gauntlet-app/src/lib.rs \
  crates/gauntlet-app/tests/conformance.rs \
  crates/gauntlet-cli/src/verify_cmd.rs \
  crates/gauntlet-cli/src/lib.rs \
  crates/gauntlet-cli/tests/conformance_cli.rs \
  crates/gauntlet-cli/tests/cli_help.txt \
  crates/gauntlet-cli/tests/cli_help_surfaces.txt \
  crates/adapters/gauntlet-adapter-verify-command-exit-code/src/lib.rs \
  crates/adapters/gauntlet-adapter-verify-command-exit-code/src/sweep_toolchain.rs \
  crates/gauntlet-arch-tests/Cargo.toml \
  crates/gauntlet-arch-tests/src/lib.rs \
  crates/gauntlet-arch-tests/src/single_writer.rs \
  crates/gauntlet-arch-tests/src/docs.rs \
  tests/contract/docs_present.rs \
  docs/ARCHITECTURE.md docs/CONFORMANCE.md docs/CONFIGURATION.md \
  docs/SECURITY.md docs/FAILURE-RECOVERY.md docs/OPERATOR-GUIDE.md \
  docs/SELF-IMPROVEMENT.md docs/HANDOFF.md \
  .agents/reports/conformance-manifest.yaml \
  .agents/reports/conformance.json \
  .agents/reports/conformance-evidence \
  .agents/reports/task27

git status --short | grep -v "^.. \.jolli" | grep -v "CURRENT_STATUS" | grep -v "gauntlet-tui" || true
echo "--- staged above; verify nothing unexpected, then commit ---"
