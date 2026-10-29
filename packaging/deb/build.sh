#!/bin/sh
# Build gauntlet_<version>_<arch>.deb from a staged native release tree.
#
# Usage: packaging/deb/build.sh <stage-dir> <version> <amd64|arm64> <out-dir>
#
# The stage dir carries the binaries under their INSTALLED names: `gauntlet`
# (required) and `gauntlet-workbench` (optional — packaged only when the
# workbench was built and launch-qualified for this platform). Nothing here
# compiles: the caller stages; this script packages. No cargo-deb, no nfpm,
# no fpm — plain dpkg-deb.
#
# Depends is computed from the staged binaries with dpkg-shlibdeps against
# the build host's package database (union across both binaries), so the
# package declares real library dependencies instead of hand-written guesses.
# Build on Ubuntu 22.04 (glibc 2.35) so the computed libc6 constraint stays
# installable on the NVIDIA Jetson AGX Orin baseline — a constraint above
# 2.35 is refused below. Recommends: postgresql, bubblewrap (both optional
# runtime capabilities, never auto-installed dependencies).
#
# The package installs (mode 0755 binaries, root:root throughout, no
# postinst, no systemd units, no service enablement):
#   /usr/bin/gauntlet
#   /usr/bin/gauntlet-workbench   (when staged)
#   /usr/share/doc/gauntlet/copyright   (the repo root LICENSE)
#   /usr/share/doc/gauntlet/README      (from packaging/deb/README.md)
#
# Repo root resolution: the GAUNTLET_REPO_ROOT environment variable when set
# and non-empty, else derived from this script's own path (../../). The
# control file is assembled from packaging/deb/control with @TOKENS@
# substituted.
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: $0 <stage-dir> <version> <amd64|arm64> <out-dir>" >&2
    exit 2
fi

stage=$(CDPATH='' cd -- "$1" && pwd)
version=$2
arch=$3
outdir=$4

case "$version" in
    '' | *[!0-9.]*)
        echo "error: invalid version '$version' (expected numeric x.y.z)" >&2
        exit 2
        ;;
esac
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "error: version '$version' is not a numeric x.y.z release; native packages cannot encode prereleases or build metadata" >&2
    exit 2
fi
case "$arch" in
    amd64 | arm64) ;;
    *)
        echo "error: unsupported architecture '$arch' (expected amd64 or arm64)" >&2
        exit 2
        ;;
esac

if [ -n "${GAUNTLET_REPO_ROOT:-}" ]; then
    repo_root=$(CDPATH='' cd -- "$GAUNTLET_REPO_ROOT" && pwd)
else
    repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
fi
deb_src_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

binary="$stage/gauntlet"
if [ ! -x "$binary" ]; then
    echo "error: missing executable $binary" >&2
    exit 1
fi
workbench_binary="$stage/gauntlet-workbench"
if [ -x "$workbench_binary" ]; then
    have_workbench=1
else
    have_workbench=0
    if [ -e "$workbench_binary" ]; then
        echo "error: $workbench_binary exists but is not executable" >&2
        exit 1
    fi
fi

sh "$repo_root/packaging/check-linux-binary.sh" "$binary" "$arch" rust
if [ "$have_workbench" -eq 1 ]; then
    sh "$repo_root/packaging/check-workbench-binary.sh" "$workbench_binary" "$arch"
fi

command -v dpkg-shlibdeps >/dev/null 2>&1 || {
    echo "error: dpkg-shlibdeps is required (install dpkg-dev)" >&2
    exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Depends from the staged binaries themselves (union across both).
mkdir -p "$work/shlibs/debian"
cat > "$work/shlibs/debian/control" <<EOF
Source: gauntlet
Section: utils
Priority: optional
Maintainer: Gauntlet maintainers <https://github.com/GodSpeedAI/Gauntlet>
Package: gauntlet
Architecture: $arch
Description: shlibs probe
 probe
EOF

shlibs_args="-e$binary"
if [ "$have_workbench" -eq 1 ]; then
    shlibs_args="$shlibs_args -e$workbench_binary"
fi

(
    cd "$work/shlibs"
    # shellcheck disable=SC2086 # one -e<binary> token per staged binary
    dpkg-shlibdeps -O $shlibs_args > "$work/shlibs.out" 2> "$work/shlibs.err"
) || {
    cat "$work/shlibs.err" >&2
    exit 1
}
deps=$(sed -n 's/^shlibs:Depends=//p' "$work/shlibs.out")
case "$deps" in
    *libc6*) ;;
    *)
        echo "error: dpkg-shlibdeps did not report libc6 (got '$deps')" >&2
        exit 1
        ;;
esac

# The Jetson baseline guard: a libc6 constraint above 2.35 means the build
# host's symbol files are newer than Ubuntu 22.04's — the deb would refuse
# to install on the supported target. Build on Ubuntu 22.04 instead.
libc6_constraint=$(
    printf '%s' "$deps" | tr ',' '\n' \
        | sed -n 's/^[[:space:]]*libc6[[:space:]]*(>=[[:space:]]*\([^)]*\)).*/\1/p' | head -1
)
if [ -n "$libc6_constraint" ]; then
    highest=$(printf '%s\n%s\n' "$libc6_constraint" '2.35' | sort -V | tail -1)
    if [ "$highest" != '2.35' ]; then
        echo "error: computed Depends requires libc6 >= $libc6_constraint, above the 2.35 Jetson baseline — build on Ubuntu 22.04" >&2
        exit 1
    fi
fi

# The package tree.
root="$work/gauntlet_${version}_${arch}"
mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/doc/gauntlet"
install -m 0755 "$binary" "$root/usr/bin/gauntlet"
if [ "$have_workbench" -eq 1 ]; then
    install -m 0755 "$workbench_binary" "$root/usr/bin/gauntlet-workbench"
fi
install -m 0644 "$repo_root/LICENSE" "$root/usr/share/doc/gauntlet/copyright"
install -m 0644 "$deb_src_dir/README.md" "$root/usr/share/doc/gauntlet/README"

installed_kib=$(du -sk "$root/usr" | cut -f1)
for token in '@VERSION@' '@ARCH@' '@INSTALLED_SIZE@' '@DEPENDS@'; do
    grep -qF "$token" "$deb_src_dir/control" || {
        echo "error: control skeleton $deb_src_dir/control is missing the $token token" >&2
        exit 1
    }
done
# Escape the sed replacement metacharacters in the computed fields.
deps_escaped=$(printf '%s' "$deps" | sed -e 's/[&|\\]/\\&/g')
sed -e "s|@VERSION@|$version|g" \
    -e "s|@ARCH@|$arch|g" \
    -e "s|@INSTALLED_SIZE@|$installed_kib|g" \
    -e "s|@DEPENDS@|$deps_escaped|g" \
    "$deb_src_dir/control" > "$root/DEBIAN/control"
chmod 0644 "$root/DEBIAN/control"

mkdir -p "$outdir"
deb="$outdir/gauntlet_${version}_${arch}.deb"
dpkg-deb --build --root-owner-group "$root" "$deb" >/dev/null

# Post-build assertions: the frozen contract is checked against the artifact,
# not assumed from the tree we just assembled.
test "$(dpkg-deb --field "$deb" Package)" = "gauntlet"
test "$(dpkg-deb --field "$deb" Version)" = "$version"
test "$(dpkg-deb --field "$deb" Architecture)" = "$arch"
test -n "$(dpkg-deb --field "$deb" Installed-Size)"
dpkg-deb --contents "$deb" > "$work/contents.txt"
for entry in usr/bin/gauntlet usr/share/doc/gauntlet/copyright usr/share/doc/gauntlet/README; do
    grep -Eq -- "-rwxr-xr-x root/root.*\./$entry\$|-rw-r--r-- root/root.*\./$entry\$" "$work/contents.txt" || {
        echo "error: $entry missing or wrong mode/owner in the built deb" >&2
        exit 1
    }
done
if [ "$have_workbench" -eq 1 ]; then
    grep -Eq -- '-rwxr-xr-x root/root.*\./usr/bin/gauntlet-workbench$' "$work/contents.txt" || {
        echo "error: ./usr/bin/gauntlet-workbench missing or wrong mode/owner in the built deb" >&2
        exit 1
    }
fi
dpkg-deb --info "$deb" | sed -n '/Package:/,/Description:/p'
echo "OK: built $deb"
