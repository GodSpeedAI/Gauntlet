#!/bin/sh
# Print the Gauntlet version — the version the `gauntlet` binary reports and
# every native package encodes — and refuse to continue when a requested
# release identity has drifted from the workspace.
#
# Usage: packaging/version.sh [requested-version-or-tag]
#
# The single source of truth is the root Cargo.toml `[workspace.package]`
# `version` key: every crate (gauntlet-cli included) inherits it, so the
# workspace version IS the CLI version. With an argument, the argument must
# name that exact release: `<version>`, `v<version>`, or the release tag
# `gauntlet-v<version>`. Anything else is refused, which is what keeps the
# package publishers from attaching artifacts built from one tree to a
# different release.
#
# Output: the x.y.z version on stdout. Exit 1 on a missing/invalid version
# or a mismatched argument. No network.
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cargo_toml="$root/Cargo.toml"

if [ ! -f "$cargo_toml" ]; then
    echo "error: workspace manifest not found: $cargo_toml" >&2
    exit 1
fi

# The `version` key of the [workspace.package] section ONLY: Cargo.toml is
# full of dependency `version = "..."` lines, so the section is tracked
# explicitly (awk; no network, no toolchain).
version=$(awk '
    /^\[workspace\.package\]/ { inpkg = 1; next }
    /^\[/                     { inpkg = 0; next }
    inpkg && $0 ~ /^[[:space:]]*version[[:space:]]*=/ {
        sub(/^[^=]*=[[:space:]]*"/, "")
        sub(/".*$/, "")
        print
        exit
    }
' "$cargo_toml")

case "$version" in
    '' | *[!0-9A-Za-z.+~-]*)
        echo "error: invalid or missing workspace version '$version' in Cargo.toml [workspace.package]" >&2
        exit 1
        ;;
esac

# The artifact builders cannot express a prerelease: `pkgbuild --version` and
# the Inno Setup `AppVersion` both require a purely numeric x.y.z, and a mixed
# release identity across platforms is worse than none. Refuse here, at the
# first gate, rather than after four platforms have compiled.
case "$version" in
    *[!0-9.]*)
        echo "error: workspace version '$version' is not a numeric x.y.z release; native packages cannot encode prereleases or build metadata" >&2
        exit 1
        ;;
esac
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "error: workspace version '$version' is not a numeric x.y.z release; native packages cannot encode prereleases or build metadata" >&2
    exit 1
fi

if [ "$#" -gt 0 ] && [ -n "$1" ]; then
    requested=$1
    requested=${requested#gauntlet-v}
    requested=${requested#v}
    if [ "$requested" != "$version" ]; then
        echo "error: requested release '$1' does not name the workspace version '$version'" >&2
        exit 1
    fi
fi

printf '%s\n' "$version"
