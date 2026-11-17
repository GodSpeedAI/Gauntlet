#!/usr/bin/env bash
# Regression checks for two helpers in packaging/qualify-local.sh:
#   sum_of            accepts bare, "*name" and "./name" SHA256SUMS entries
#   preserve_gh_auth  keeps gh authenticated across the tarball-mode HOME redirect
# Pure shell; no network, no gh, no credentials. Run: bash packaging/test-qualify-local.sh
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
script="$here/qualify-local.sh"
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
fail=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1" >&2; fail=1; }

# Load just the helper definitions from the script under test.
# shellcheck disable=SC1090
source <(sed -n '/^preserve_gh_auth() {/,/^}/p;/^sum_of() {/,/^}/p' "$script")

h1=$(printf a | sha256sum | cut -d' ' -f1); h2=$(printf b | sha256sum | cut -d' ' -f1)
printf '%s  x.deb\n%s *y.tar.gz\n' "$h1" "$h2" >"$t/bare"
printf '%s  ./x.deb\n%s  ./y.tar.gz\n' "$h1" "$h2" >"$t/dot"
for f in bare dot; do
    [ "$(sum_of "$t/$f" x.deb)" = "$h1" ] && ok "sum_of $f x.deb" || bad "sum_of $f x.deb"
    [ "$(sum_of "$t/$f" y.tar.gz)" = "$h2" ] && ok "sum_of $f y.tar.gz" || bad "sum_of $f y.tar.gz"
    [ -z "$(sum_of "$t/$f" missing.deb)" ] && ok "sum_of $f missing" || bad "sum_of $f missing"
done
[ -z "$(sum_of "$t/nofile" x.deb || true)" ] && ok "sum_of no file" || bad "sum_of no file"

# preserve_gh_auth: real config dir exported as a path, nothing copied.
mkdir -p "$t/real/.config/gh"; echo "oauth_token: SECRET" >"$t/real/.config/gh/hosts.yml"
( unset GH_CONFIG_DIR XDG_CONFIG_HOME; HOME="$t/real"; preserve_gh_auth
  [ "${GH_CONFIG_DIR:-}" = "$t/real/.config/gh" ] ) && ok "gh dir exported" || bad "gh dir exported"
( unset XDG_CONFIG_HOME; GH_CONFIG_DIR=/custom; HOME="$t/real"; preserve_gh_auth; [ "$GH_CONFIG_DIR" = /custom ] ) \
    && ok "explicit GH_CONFIG_DIR kept" || bad "explicit GH_CONFIG_DIR kept"
( unset GH_CONFIG_DIR XDG_CONFIG_HOME; HOME="$t/none"; preserve_gh_auth; [ -z "${GH_CONFIG_DIR:-}" ] ) \
    && ok "no gh dir, nothing exported" || bad "no gh dir, nothing exported"
# the redirected HOME must contain no copied credentials
mkdir -p "$t/redir"; [ -z "$(find "$t/redir" -type f)" ] && ok "no credential copy" || bad "no credential copy"
# the script must call it before redirecting HOME
awk '/preserve_gh_auth$/{p=NR} /export HOME="\$work\/home"/{h=NR} END{exit !(p && h && p<h)}' "$script" \
    && ok "called before HOME redirect" || bad "called before HOME redirect"
# --previous-dir must be absolute before apt-get sees it: a relative "previous/x.deb"
# reaches apt-get as a package name ("Unable to locate package previous").
grep -q 'prev_dir=$(CDPATH=.* cd -- "$prev_dir" && pwd)' "$script" \
    && ok "previous-dir normalized to an absolute path" || bad "previous-dir normalized to an absolute path"
exit $fail
