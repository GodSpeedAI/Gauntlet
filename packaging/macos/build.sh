#!/bin/sh
# Build the macOS install package (.pkg) and disk image (.dmg) for Gauntlet.
#
# Usage: packaging/macos/build.sh <arm64-stage-dir> [<x64-stage-dir>] <version> <out-dir>
#
# Each stage dir must contain BOTH installed-name binaries: gauntlet (the
# Rust CLI) and gauntlet-workbench (the Bun standalone executable). With only
# the arm64 stage the package is named -macos-arm64; with both stages each
# binary pair is merged with lipo and the package is named -macos-universal.
# An arm64-only build is NEVER labeled universal: the label follows the lipo
# outcome, not the intent.
#
# The package is UNSIGNED: no Developer ID certificate is used, and none is
# claimed anywhere. Every staged binary is ad-hoc signed (--force --sign -)
# because arm64 macOS requires a valid (ad-hoc is valid) signature to run.
#
# Payload layout (with --install-location /usr/local):
#   bin/gauntlet                  -> /usr/local/bin/gauntlet
#   bin/gauntlet-workbench        -> /usr/local/bin/gauntlet-workbench
#   share/doc/gauntlet/LICENSE
#   share/doc/gauntlet/README-install.txt
#
# Pattern adapted from the proven sxr/domainforge/sea-rs
# packaging/macos/build.sh (pkgbuild /usr/local, ad-hoc codesign, UDZO dmg).
set -eu

if [ "$#" -ne 3 ] && [ "$#" -ne 4 ]; then
    echo "usage: $0 <arm64-stage-dir> [<x64-stage-dir>] <version> <out-dir>" >&2
    exit 2
fi
arm_dir=$1
if [ "$#" -eq 4 ]; then
    x86_dir=$2
    version=$3
    outdir=$4
else
    x86_dir=''
    version=$2
    outdir=$3
fi

case "$version" in
    '' | *[!0-9A-Za-z.+~-]*)
        echo "error: invalid version '$version'" >&2
        exit 2
        ;;
esac

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

# Both installed-name binaries are required in every provided stage dir.
for bin in gauntlet gauntlet-workbench; do
    if [ ! -x "$arm_dir/$bin" ]; then
        echo "error: missing executable $arm_dir/$bin" >&2
        exit 1
    fi
    if [ -n "$x86_dir" ] && [ ! -x "$x86_dir/$bin" ]; then
        echo "error: missing executable $x86_dir/$bin" >&2
        exit 1
    fi
done

if [ -n "$x86_dir" ]; then
    label=universal
else
    label=arm64
fi
pkg_name="gauntlet-${version}-macos-${label}.pkg"
dmg_name="gauntlet-${version}-macos-${label}.dmg"
identifier=ai.godspeed.gauntlet

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Payload: bin/ installs to /usr/local/bin; share/doc to /usr/local/share/doc.
stage="$work/stage"
mkdir -p "$stage/bin" "$stage/share/doc/gauntlet"
for bin in gauntlet gauntlet-workbench; do
    if [ -n "$x86_dir" ]; then
        lipo -create "$arm_dir/$bin" "$x86_dir/$bin" -output "$stage/bin/$bin"
    else
        cp "$arm_dir/$bin" "$stage/bin/$bin"
    fi
    chmod 0755 "$stage/bin/$bin"
    # Ad-hoc signature on every staged binary (required on arm64; harmless
    # on x86_64). A lipo merge invalidates input signatures, so signing
    # happens after staging, never before.
    codesign --force --sign - "$stage/bin/$bin" >/dev/null
done

# Verify the staged architectures match the label honestly.
for bin in gauntlet gauntlet-workbench; do
    archs=$(lipo -archs "$stage/bin/$bin")
    echo "lipo -archs $bin: $archs"
    case " $archs " in
        *" arm64 "*)
            ;;
        *)
            echo "error: staged $bin lacks an arm64 slice (lipo -archs: '$archs')" >&2
            exit 1
            ;;
    esac
    if [ -n "$x86_dir" ]; then
        case " $archs " in
            *" x86_64 "*)
                ;;
            *)
                echo "error: universal build but staged $bin lacks an x86_64 slice (lipo -archs: '$archs')" >&2
                exit 1
                ;;
        esac
    fi
done

install -m 0644 "$repo_root/LICENSE" "$stage/share/doc/gauntlet/LICENSE"

# Install notes: written before pkgbuild so the payload genuinely carries
# them, then copied onto the disk image alongside the package.
readme="$work/README-install.txt"
cat > "$readme" <<EOF
Gauntlet ${version} for macOS (${label})

Install
  Double-click ${pkg_name}, or from Terminal:
    sudo installer -pkg ${pkg_name} -target /
  The commands gauntlet and gauntlet-workbench are placed in /usr/local/bin.

Unsigned package
  This package is NOT signed or notarized (no Apple Developer ID is used).
  Gatekeeper may refuse to open it. Either:
    - Control-click the package, choose Open, and confirm; or
    - remove the quarantine flag after download:
        xattr -d com.apple.quarantine ${pkg_name}
  Verify the download before installing:
    shasum -a 256 -c SHA256SUMS
    gh attestation verify ${pkg_name} --repo GodSpeedAI/Gauntlet

Runtime prerequisites
  A PostgreSQL server is REQUIRED for run/status/inspect/verify/export/
  serve/fabric: point GAUNTLET_DATABASE_URL at a real server before using
  them (the doctor command does not probe PostgreSQL; absence is reported
  honestly, not simulated). The bubblewrap sandbox is optional (default
  backend is none; a configured non-none backend without bwrap fails
  closed with a typed refusal). The Bun runtime is optional and only
  needed for the codemode capability; the gauntlet-workbench executable is
  self-contained (Bun compiled standalone).

Workbench
  gauntlet-workbench is the operator TUI. It connects to the operator
  surface through the Gauntlet gateway; set GAUNTLET_ENDPOINT (or
  GAUNTLET_WORKBENCH_ENDPOINT) and GAUNTLET_WORKBENCH_GAUNTLET_BINARY to
  point it at your CLI. It needs a writable working directory for its
  projection store (or set GAUNTLET_WORKBENCH_PROJECTION_STORE).

Uninstall
  sudo rm -f /usr/local/bin/gauntlet /usr/local/bin/gauntlet-workbench
  sudo rm -rf /usr/local/share/doc/gauntlet
  sudo pkgutil --forget ${identifier}

Licence: see LICENSE in this image.
EOF
install -m 0644 "$readme" "$stage/share/doc/gauntlet/README-install.txt"

mkdir -p "$outdir"
pkgbuild --root "$stage" \
    --identifier "$identifier" \
    --version "$version" \
    --install-location /usr/local \
    "$outdir/$pkg_name" >/dev/null

rm -f "$outdir/$dmg_name"
dmg_src="$work/dmg"
mkdir -p "$dmg_src"
cp "$outdir/$pkg_name" "$dmg_src/"
cp "$repo_root/LICENSE" "$dmg_src/LICENSE"
cp "$readme" "$dmg_src/README-install.txt"

hdiutil create -volname "Gauntlet ${version}" -srcfolder "$dmg_src" \
    -format UDZO "$outdir/$dmg_name" >/dev/null

# Post-build assertions: the package carries the requested version and
# identifier (read from the expanded PackageInfo plist; there is no plain
# `pkg --info` tool on macOS), and both outputs are non-empty.
pkg_expand="$work/pkginfo"
rm -rf "$pkg_expand"
pkgutil --expand "$outdir/$pkg_name" "$pkg_expand" \
    || { echo "error: could not expand $outdir/$pkg_name for verification" >&2; exit 1; }
if ! grep -Eq " version=\"$version\"" "$pkg_expand/PackageInfo"; then
    echo "error: PackageInfo does not show version $version for $outdir/$pkg_name" >&2
    exit 1
fi
if ! grep -Fq "identifier=\"$identifier\"" "$pkg_expand/PackageInfo"; then
    echo "error: PackageInfo identifier is not $identifier for $outdir/$pkg_name" >&2
    exit 1
fi
test -s "$outdir/$pkg_name" || { echo "error: empty or missing $outdir/$pkg_name" >&2; exit 1; }
test -s "$outdir/$dmg_name" || { echo "error: empty or missing $outdir/$dmg_name" >&2; exit 1; }

echo "built $outdir/$pkg_name"
echo "built $outdir/$dmg_name"
echo "sha256 ($pkg_name): $(shasum -a 256 "$outdir/$pkg_name" | awk '{print $1}')"
echo "sha256 ($dmg_name): $(shasum -a 256 "$outdir/$dmg_name" | awk '{print $1}')"
