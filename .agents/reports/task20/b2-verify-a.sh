#!/bin/bash
# B2 recovery verification, group A (fast gates). Run under flock from repo root.
cd /home/sprime01/projects/gauntlet || exit 9
echo "=== VERIFY-A start $(date) ==="
just ruler; echo "RULER_EXIT=$?"
just doctor; echo "DOCTOR_EXIT=$?"
echo "--- dry-run gate ---"
git status --porcelain > /tmp/b2-verify-before.txt
cargo run -q -p gauntlet-cli -- run --dry-run "seeded bounded task"
echo "DRYRUN_EXIT=$?"
git status --porcelain > /tmp/b2-verify-after.txt
diff /tmp/b2-verify-before.txt /tmp/b2-verify-after.txt && echo "ZERO_WRITES_CONFIRMED" || echo "WRITES_DETECTED"
echo "--- just check ---"
just check; echo "CHECK_EXIT=$?"
echo "=== VERIFY-A end $(date) ==="
