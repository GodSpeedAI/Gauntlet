#!/bin/sh
# Build the macOS install package (.pkg) and disk image (.dmg) for Gauntlet.
#
# Usage: packaging/macos/build.sh <arm64-stage-dir> [<x64-stage-dir>] <version> <out-dir>
#
# Composition contract (F5: the macOS pkg NEVER contains an x86_64 workbench
# slice — an unqualified cross-built workbench must not ride along as though
# it passed release validation):
#
#   gauntlet (CLI)      REQUIRED in the arm64 stage, and REQUIRED in the x64
#                       stage when that stage is provided. With both stages
#                       the CLI pair is lipo-merged into a universal slice
#                       and the package is named -macos-universal; with only
#                       the arm64 stage it is installed arm64-only and the
#                       package is named -macos-arm64.
#   gauntlet-workbench  OPTIONAL, taken ONLY from the arm64 stage and
#                       installed AS-IS — never lipo'd, never x86_64. If the
#                       x64 stage contains a gauntlet-workbench binary this
#                       script REFUSES with a loud error: a darwin-x64
#                       workbench may only ship as a separate unqualified
#                       artifact handled by the workflow, never inside this
#                       package.
#
# The README-install.txt carries a Composition line stating exactly which
# binaries are inside and their architectures.
#
# The package is UNSIGNED: no Developer ID certificate is used, and none is
# claimed anywhere. Every staged binary is ad-hoc signed (--force --sign -)
# because arm64 macOS requires a valid (ad-hoc is valid) signature to run.
#
# Payload layout (with --install-location /usr/local):
#   bin/gauntlet                  -> /usr/local/bin/gauntlet
#   bin/gauntlet-workbench        -> /usr/local/bin/gauntlet-workbench
#                                    (only when staged from the arm64 dir)
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

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)

# Stage validation: the CLI is required per stage; the workbench is optional
# and may come ONLY from the arm64 stage.
if [ ! -x "$arm_dir/gauntlet" ]; then
    echo "error: missing executable $arm_dir/gauntlet" >&2
    exit 1
fi
if [ -n "$x86_dir" ] && [ ! -x "$x86_dir/gauntlet" ]; then
    echo "error: missing executable $x86_dir/gauntlet" >&2
    exit 1
fi
if [ -e "$x86_dir/gauntlet-workbench" ]; then
    echo "error: refusing to package $x86_dir/gauntlet-workbench: the macOS package" >&2
    echo "error: NEVER contains an x86_64 workbench slice. A darwin-x64 workbench may" >&2
    echo "error: only ship as a separate unqualified artifact (workflow-handled), never" >&2
    echo "error: inside a universal-labeled package." >&2
    exit 1
fi
if [ -x "$arm_dir/gauntlet-workbench" ]; then
    workbench_staged=1
else
    workbench_staged=0
fi

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

# CLI: universal when both stages are present, else arm64-only.
if [ -n "$x86_dir" ]; then
    lipo -create "$arm_dir/gauntlet" "$x86_dir/gauntlet" -output "$stage/bin/gauntlet"
else
    cp "$arm_dir/gauntlet" "$stage/bin/gauntlet"
fi
chmod 0755 "$stage/bin/gauntlet"
# Ad-hoc signature (required on arm64; harmless on x86_64). A lipo merge
# invalidates input signatures, so signing happens after staging.
codesign --force --sign - "$stage/bin/gauntlet" >/dev/null

# Workbench: arm64 stage only, installed as-is (never lipo'd).
if [ "$workbench_staged" -eq 1 ]; then
    cp "$arm_dir/gauntlet-workbench" "$stage/bin/gauntlet-workbench"
    chmod 0755 "$stage/bin/gauntlet-workbench"
    codesign --force --sign - "$stage/bin/gauntlet-workbench" >/dev/null
fi

# Verify the staged architectures match the composition honestly: the CLI is
# arm64 (+ x86_64 exactly when universal); the workbench, when staged, is
# arm64-only — an x86_64 slice inside the workbench is refused.
cli_archs=$(lipo -archs "$stage/bin/gauntlet")
echo "lipo -archs gauntlet: $cli_archs"
case " $cli_archs " in
    *" arm64 "*)
        ;;
    *)
        echo "error: staged gauntlet lacks an arm64 slice (lipo -archs: '$cli_archs')" >&2
        exit 1
        ;;
esac
if [ -n "$x86_dir" ]; then
    case " $cli_archs " in
        *" x86_64 "*)
            ;;
        *)
            echo "error: universal build but staged gauntlet lacks an x86_64 slice (lipo -archs: '$cli_archs')" >&2
            exit 1
            ;;
    esac
fi
if [ "$workbench_staged" -eq 1 ]; then
    wb_archs=$(lipo -archs "$stage/bin/gauntlet-workbench")
    echo "lipo -archs gauntlet-workbench: $wb_archs"
    case " $wb_archs " in
        *" arm64 "*)
            ;;
        *)
            echo "error: staged gauntlet-workbench lacks an arm64 slice (lipo -archs: '$wb_archs')" >&2
            exit 1
            ;;
    esac
    case " $wb_archs " in
        *" x86_64 "*)
            echo "error: staged gauntlet-workbench carries an x86_64 slice; the macOS package" >&2
            echo "error: never ships one — the workbench must be a thin arm64 binary" >&2
            exit 1
            ;;
    esac
fi

install -m 0644 "$repo_root/LICENSE" "$stage/share/doc/gauntlet/LICENSE"

# Composition facts for the README: state exactly which binaries are inside
# and their arch(es); never claim a capability the payload does not carry.
if [ -n "$x86_dir" ]; then
    cli_arch_text="universal (arm64 + x86_64)"
else
    cli_arch_text="arm64"
fi
if [ "$workbench_staged" -eq 1 ]; then
    if [ -n "$x86_dir" ]; then
        composition="CLI: gauntlet (${cli_arch_text}); workbench: gauntlet-workbench (arm64 only)."
        wb_note="The included gauntlet-workbench runs on Apple Silicon only; Intel Macs get the CLI only. No x86_64 workbench slice is ever shipped in this package."
    else
        composition="CLI: gauntlet (${cli_arch_text}); workbench: gauntlet-workbench (arm64)."
        wb_note="Both binaries in this package are arm64."
    fi
else
    composition="CLI: gauntlet (${cli_arch_text}); workbench: not included."
    wb_note="This package does NOT include gauntlet-workbench: the macOS workbench was not staged/qualified for this build, so it is not shipped here as though it passed release validation."
fi

# Install notes: written before pkgbuild so the payload genuinely carries
# them, then copied onto the disk image alongside the package.
readme="$work/README-install.txt"
{
    cat <<EOF
Gauntlet ${version} for macOS (${label})

Composition
  ${composition}
  ${wb_note}

Install
  Double-click ${pkg_name}, or from Terminal:
    sudo installer -pkg ${pkg_name} -target /
  The gauntlet command is placed in /usr/local/bin.$(if [ "$workbench_staged" -eq 1 ]; then printf ' The gauntlet-workbench TUI is placed there as well.'; fi)

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
  needed for the codemode capability; the packaged binaries are
  self-contained.
EOF
    if [ "$workbench_staged" -eq 1 ]; then
        cat <<EOF

Workbench
  gauntlet-workbench is the operator TUI. It connects to the operator
  surface through the Gauntlet gateway; set GAUNTLET_ENDPOINT (or
  GAUNTLET_WORKBENCH_ENDPOINT) and GAUNTLET_WORKBENCH_GAUNTLET_BINARY to
  point it at your CLI. It needs a writable working directory for its
  projection store (or set GAUNTLET_WORKBENCH_PROJECTION_STORE).
EOF
    fi
    cat <<EOF

Uninstall
  sudo rm -f /usr/local/bin/gauntlet /usr/local/bin/gauntlet-workbench
  sudo rm -rf /usr/local/share/doc/gauntlet
  sudo pkgutil --forget ${identifier}

Licence: see LICENSE in this image.
EOF
} > "$readme"
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
