#!/bin/sh
# The workbench flavor of the Linux ABI gate: the `gauntlet-workbench` Bun
# standalone executable is held to the same glibc 2.35 baseline as the CLI,
# with the Bun runtime's extra libresolv dependency admitted.
#
# Usage: packaging/check-workbench-binary.sh <binary> <amd64|arm64>
#
# A thin wrapper over packaging/check-linux-binary.sh with kind=bun; kept as
# its own entrypoint because the CI workflow (and the plan's interface
# contract) invokes exactly this name for the workbench artifact.
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: packaging/check-workbench-binary.sh <binary> <amd64|arm64>" >&2
    exit 2
fi

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
exec sh "$here/check-linux-binary.sh" "$1" "$2" bun
