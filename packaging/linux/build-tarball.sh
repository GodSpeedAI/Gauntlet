#!/bin/sh
# Build the self-contained Linux tarball gauntlet-<version>-<triple>.tar.gz;
# independent of the .deb.
#
# Usage: packaging/linux/build-tarball.sh <stage-dir> <version> <target-triple> <out-dir>
#
# The stage dir carries the binaries under their INSTALLED names: `gauntlet`
# (required) and `gauntlet-workbench` (optional). The archive layout is:
#
#   gauntlet-<version>-<triple>/
#     gauntlet            (0755)
#     gauntlet-workbench  (0755, when staged)
#     LICENSE             (the repo root LICENSE)
#     README.txt          (generated: install + runtime prerequisites)
#
# The archive is reproducible: normalized sort order, ownership, timestamps,
# and gzip -n (no embedded name or mtime); the script rebuilds it a second
# time and byte-compares before declaring success.
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: $0 <stage-dir> <version> <target-triple> <out-dir>" >&2
    exit 2
fi

stage=$(CDPATH='' cd -- "$1" && pwd)
version=$2
target=$3
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
case "$target" in
    x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu) ;;
    *)
        echo "error: unsupported target '$target'" >&2
        exit 2
        ;;
esac

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

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
[ -f "$repo_root/LICENSE" ] || {
    echo "error: LICENSE not found at $repo_root/LICENSE" >&2
    exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

name="gauntlet-${version}-${target}"

build_archive() {
    # $1 = destination .tar.gz path
    dest=$1
    rm -rf "$work/tree"
    stage_dir="$work/tree/$name"
    mkdir -p "$stage_dir"
    install -m 0755 "$binary" "$stage_dir/gauntlet"
    if [ "$have_workbench" -eq 1 ]; then
        install -m 0755 "$workbench_binary" "$stage_dir/gauntlet-workbench"
    fi
    install -m 0644 "$repo_root/LICENSE" "$stage_dir/LICENSE"
    cat > "$stage_dir/README.txt" <<EOF
Gauntlet ${version} (${target})

This archive contains the standalone \`gauntlet\` command-line runtime and,
when present, the \`gauntlet-workbench\` terminal operator surface (a Bun
standalone executable). Both are glibc-linked ELF binaries and need a glibc
2.35-or-newer Linux system (Ubuntu 22.04 or newer — e.g. the NVIDIA Jetson
AGX Orin). No Rust toolchain or other language runtime is required.

Install (pick one, and install the workbench beside it when present):
  install -m 0755 gauntlet \$HOME/.local/bin/gauntlet
  sudo install -m 0755 gauntlet /usr/local/bin/gauntlet

REQUIRED runtime prerequisite — PostgreSQL:
  gauntlet run/status/inspect/verify/export/serve need a PostgreSQL server.
  Point GAUNTLET_DATABASE_URL at a real server before use, for example:
    export GAUNTLET_DATABASE_URL=postgresql://gauntlet:secret@localhost:5432/gauntlet
  The built-in default is a DEVELOPER instance (postgres:postgres on
  127.0.0.1:54329) that does not exist on user machines — always set the
  variable explicitly.

Optional capabilities (reported honestly by \`gauntlet doctor\`):
  bubblewrap  enables non-\`none\` sandbox backends. The default backend is
              \`none\`; requesting another backend without bubblewrap is a
              typed refusal, never a silent fallback.
  bun         needed only by the codemode tool-program host.

Launching:
  Commands resolve the Gauntlet project from the current working directory
  (the directory holding GAUNTLET.md); GAUNTLET_PROJECT_ROOT overrides it.
  Launch from a writable project directory — \`gauntlet init\` scaffolds one.
  Reports land under the project; GAUNTLET_REPORT_DIR redirects them.
  First diagnostic: ./gauntlet doctor

Workbench (when present):
  The workbench is a projection of the §20.2 operator surface — it connects
  to the gateway, never bypasses it. Start the surface with the CLI (bind
  via GAUNTLET_API_BIND, loopback only) and point the workbench at it:
    export GAUNTLET_API_BIND=127.0.0.1:4000
    ./gauntlet serve
    export GAUNTLET_ENDPOINT=http://127.0.0.1:4000
    ./gauntlet-workbench

Licence: see LICENSE.
EOF

    mkdir -p "$(dirname -- "$dest")"
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
        -C "$work/tree" -cf - "$name" | gzip -9n > "$dest"
}

build_archive "$work/first.tar.gz"
build_archive "$work/second.tar.gz"
cmp "$work/first.tar.gz" "$work/second.tar.gz" || {
    echo "error: the tarball is not reproducible (two builds differ)" >&2
    exit 1
}

mkdir -p "$outdir"
archive="$outdir/${name}.tar.gz"
mv "$work/first.tar.gz" "$archive"

test -s "$archive"
tar -tzf "$archive" | grep -q "^${name}/gauntlet\$"
tar -tzf "$archive" | grep -q "^${name}/LICENSE\$"
tar -tzf "$archive" | grep -q "^${name}/README.txt\$"
if [ "$have_workbench" -eq 1 ]; then
    tar -tzf "$archive" | grep -q "^${name}/gauntlet-workbench\$"
fi
echo "OK: built $archive (reproducible across two builds)"
