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
