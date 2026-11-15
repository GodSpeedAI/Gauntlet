#!/bin/sh
# Fail if a staged directory or a built package carries Prime Agent.
#
# Prime Agent is an OPTIONAL dependency that Gauntlet never bundles and never
# installs (WP-63). Usage: packaging/assert-no-prime-agent.sh <path>...
# Each path is a directory (entry names are checked), a .tar.gz/.tgz, a .deb,
# or a .zip (the archive listing is checked). Any other file kind (.pkg, .dmg,
# setup.exe) cannot be listed portably: only its own name is checked, and the
# staging directory that fed it must be passed too.
#
# The match is on ENTRY NAMES (prime-agent, prime_agent, share/prime). The
# gauntlet binary itself legitimately contains the adapter token as data, so
# file contents are never scanned.
set -eu

if [ "$#" -lt 1 ]; then
    echo "usage: packaging/assert-no-prime-agent.sh <dir-or-package>..." >&2
    exit 2
fi

pattern='prime[-_]agent|share/prime|/prime$|^prime$'
bad=0

# check <label> <listing>: runs in this shell (never in a pipeline), so a
# failure is recorded in $bad.
check() {
    if printf '%s\n' "$2" | grep -Eiq "$pattern"; then
        echo "FAIL: $1 lists Prime Agent content (it must never be packaged)" >&2
        bad=1
    else
        echo "ok: $1 carries no Prime Agent entries"
    fi
}

for target in "$@"; do
    if [ -d "$target" ]; then
        check "$target" "$(cd "$target" && find . -print)"
        continue
    fi
    if [ ! -f "$target" ]; then
        echo "FAIL: $target does not exist" >&2
        bad=1
        continue
    fi
    case "$target" in
        *.tar.gz|*.tgz) check "$target" "$(tar -tzf "$target")" ;;
        *.deb) check "$target" "$(dpkg-deb -c "$target")" ;;
        *.zip)
            if command -v unzip >/dev/null 2>&1; then
                check "$target" "$(unzip -Z1 "$target")"
            else
                check "$target" "$(python3 -m zipfile -l "$target")"
            fi
            ;;
        *) check "$target (name only)" "$(basename "$target")" ;;
    esac
done

exit "$bad"
