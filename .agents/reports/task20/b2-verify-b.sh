#!/bin/bash
# B2 recovery verification, group B (long gates). Run under flock from repo root.
cd /home/sprime01/projects/gauntlet || exit 9
echo "=== VERIFY-B start $(date) ==="
just test; echo "TEST_EXIT=$?"
just lint; echo "LINT_EXIT=$?"
just deny; echo "DENY_EXIT=$?"
just boundaries; echo "BOUNDARIES_EXIT=$?"
echo "=== VERIFY-B end $(date) ==="
