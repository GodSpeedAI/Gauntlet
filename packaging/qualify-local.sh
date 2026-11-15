#!/usr/bin/env bash
# Release qualification of the PACKAGED Gauntlet artifacts on THIS host, the
# same script CI runs on hosted runners and an operator runs on a Jetson.
# It installs the artifact the way a user would, smokes it, runs one governed
# mock-adapter loop, upgrades from the previous release when one is supplied,
# uninstalls, and asserts nothing is left behind. It writes one JSON record
# (see packaging/qualify.py) with every step PASS / FAIL / NOT_RUN and its
# reason. It never skips silently and never marks a step PASS it did not run.
#
# Usage:
#   packaging/qualify-local.sh --platform <id> --dist <dir> --version <x.y.z> \
#       --commit <sha> --out <record.json> [--runner-image <name>] \
#       [--previous-dir <dir> --previous-version <x.y.z>] [--attest-repo <owner/repo>] \
#       [--mode package|tarball]
#
#   --mode package (default): install with the OS installer (needs sudo on Linux/macOS).
#   --mode tarball (Linux only): NO sudo, NO system changes. The tarball is
#       installed into a prefix inside the scratch dir, HOME and XDG_* are
#       redirected into the scratch dir, and the .deb install/upgrade steps are
#       recorded NOT_RUN. The record status is "QUALIFIED (tarball)" at best.
#       Point TMPDIR at a scratch dir (e.g. TMPDIR=$(mktemp -d ~/gauntlet-qual.XXXX));
#       everything is removed at exit. An already-installed system gauntlet is
#       never touched.
#
#   <id>: windows-x64 | linux-amd64 | linux-arm64-generic | linux-arm64-jetson
#         | macos-arm64 | macos-x64
#   linux-arm64-jetson is accepted ONLY on a Jetson (L4T) host and is the only
#   id whose record counts as real-hardware evidence.
# <dir> holds the downloaded release assets (the .deb / .tar.gz / .pkg /
# -setup.exe) and SHA256SUMS (combined, or a SHA256SUMS-<platform> file).
#
# Environment (optional):
#   GAUNTLET_DATABASE_URL  reachable PostgreSQL for the governed-run step; when
#                          unset the step is recorded NOT_RUN with that reason.
#   GH_TOKEN               enables `gh attestation verify` when --attest-repo is set.
# Linux and macOS need sudo (or root); Windows installs per user.
set -uo pipefail

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$here/.." && pwd)

mode=package platform="" dist="" version="" commit="" out="" runner_image="" prev_dir="" prev_version="" attest_repo=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --platform) platform=$2 ;; --dist) dist=$2 ;; --version) version=$2 ;;
        --commit) commit=$2 ;; --out) out=$2 ;; --runner-image) runner_image=$2 ;;
        --previous-dir) prev_dir=$2 ;; --previous-version) prev_version=$2 ;;
        --attest-repo) attest_repo=$2 ;; --mode) mode=$2 ;;
        *) echo "qualify: unknown argument $1" >&2; exit 2 ;;
    esac
    shift 2 || { echo "qualify: $1 needs a value" >&2; exit 2; }
done
for v in platform dist version commit out; do
    [ -n "${!v}" ] || { echo "qualify: --${v} is required" >&2; exit 2; }
done
case "$mode" in package|tarball) ;; *) echo "qualify: --mode must be package or tarball" >&2; exit 2 ;; esac
dist=$(CDPATH='' cd -- "$dist" && pwd)
case "$out" in /*|[A-Za-z]:*) ;; *) out="$PWD/$out" ;; esac

PY=$(command -v python3 || command -v python || true)
[ -n "$PY" ] || { echo "qualify: python3 is required" >&2; exit 2; }

case "$platform" in
    windows-x64) os=windows; arch=x86_64 ;;
    linux-amd64) os=linux; arch=amd64; triple=x86_64-unknown-linux-gnu ;;
    linux-arm64-generic|linux-arm64-jetson) os=linux; arch=arm64; triple=aarch64-unknown-linux-gnu ;;
    macos-arm64|macos-x64) os=macos; arch=${platform#macos-} ;;
    *) echo "qualify: unknown platform $platform" >&2; exit 2 ;;
esac

detail=""
evidence="operator-host"
[ "${GITHUB_ACTIONS:-}" = "true" ] && evidence="hosted-runner"
if [ "$platform" = "linux-arm64-jetson" ]; then
    if [ -r /etc/nv_tegra_release ] || grep -qi 'nvidia jetson\|tegra' /proc/device-tree/model 2>/dev/null; then
        evidence="real-hardware"
        detail="$(tr -d '\0' < /proc/device-tree/model 2>/dev/null); $(head -1 /etc/nv_tegra_release 2>/dev/null)"
        [ "${GITHUB_ACTIONS:-}" = "true" ] && { echo "qualify: a hosted runner is never Jetson evidence" >&2; exit 2; }
    else
        echo "qualify: --platform linux-arm64-jetson requires a Jetson (L4T) host; use linux-arm64-generic elsewhere" >&2
        exit 2
    fi
fi
[ -n "$runner_image" ] || runner_image="${ImageOS:-}${ImageVersion:+ $ImageVersion}"
[ -n "$runner_image" ] || runner_image="$(uname -srm)"

work=$(mktemp -d)
steps="$work/steps.tsv"
: > "$steps"
data="$work/userdata"   # user data created by the governed run; uninstall must not touch it
mkdir -p "$data"
reasonf="$work/reason"
prefix="$work/prefix"
if [ "$mode" = tarball ]; then
    [ "$os" = linux ] || { echo "qualify: --mode tarball is Linux only" >&2; exit 2; }
    # Isolate every user-level path so the operator's real data is never read or written.
    mkdir -p "$work/home" "$prefix/bin"
    export HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" XDG_DATA_HOME="$work/home/.local/share" \
        XDG_STATE_HOME="$work/home/.local/state" XDG_CACHE_HOME="$work/home/.cache"
fi
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

rec() { # name status reason
    printf '%s\t%s\t%s\n' "$1" "$2" "$(printf '%s' "$3" | tr '\t\r\n' '   ' | cut -c1-600)" >> "$steps"
    echo "qualify: [$2] $1 $3"
}
# step <name> <function>: the function returns 0 (PASS), 77 (NOT_RUN, reason
# in $reasonf) or anything else (FAIL, reason in $reasonf or the log tail).
step() {
    local name=$1 fn=$2 rc
    : > "$reasonf"
    echo "qualify: ---- $name ----"
    "$fn" > "$work/$name.log" 2>&1
    rc=$?
    tail -n 25 "$work/$name.log"
    local reason; reason=$(cat "$reasonf" 2>/dev/null)
    case $rc in
        0) rec "$name" PASS "$reason" ;;
        77) rec "$name" NOT_RUN "${reason:-not run}" ;;
        *) [ -n "$reason" ] || reason=$(tail -n 3 "$work/$name.log" | tr '\n' ' ')
           rec "$name" FAIL "$reason" ;;
    esac
}
why() { printf '%s' "$1" > "$reasonf"; }
sudo_() { if [ "$(id -u)" = 0 ] || [ "$os" = windows ]; then "$@"; else sudo "$@"; fi; }

# ── artifacts ────────────────────────────────────────────────────────────
case "$os" in
    linux)
        deb="$dist/gauntlet_${version}_${arch}.deb"
        tarball="$dist/gauntlet-${version}-${triple}.tar.gz"
        artifact_files="$deb $tarball"
        gaunt=/usr/bin/gauntlet; wbench=/usr/bin/gauntlet-workbench
        if [ "$mode" = tarball ]; then gaunt="$prefix/bin/gauntlet"; wbench="$prefix/bin/gauntlet-workbench"; fi ;;
    macos)
        pkg=$(ls "$dist"/gauntlet-"${version}"-macos-*.pkg 2>/dev/null | head -1)
        dmg=$(ls "$dist"/gauntlet-"${version}"-macos-*.dmg 2>/dev/null | head -1)
        artifact_files="$pkg $dmg"
        gaunt=/usr/local/bin/gauntlet; wbench=/usr/local/bin/gauntlet-workbench ;;
    windows)
        setup="$dist/gauntlet-${version}-x86_64-pc-windows-msvc-setup.exe"
        zipf="$dist/gauntlet-${version}-x86_64-pc-windows-msvc.zip"
        artifact_files="$setup $zipf"
        appdir_u="$(cygpath -u "$LOCALAPPDATA")/Programs/Gauntlet"
        gaunt="$appdir_u/gauntlet.exe"; wbench="$appdir_u/gauntlet-workbench.exe" ;;
esac

artifacts_meta=""
for f in $artifact_files; do
    [ -f "$f" ] || continue
    sha=$("$PY" -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$f")
    artifacts_meta="${artifacts_meta}$(basename "$f")=${sha};"
done

# ── step: checksums ──────────────────────────────────────────────────────
s_checksums() {
    "$PY" - "$dist" $artifact_files <<'PYEOF' || return 1
import glob, hashlib, os, sys
dist, files = sys.argv[1], sys.argv[2:]
sums = {}
for p in [os.path.join(dist, "SHA256SUMS")] + sorted(glob.glob(os.path.join(dist, "SHA256SUMS-*"))):
    if os.path.isfile(p):
        for line in open(p, encoding="utf-8"):
            parts = line.strip().split(None, 1)
            if len(parts) == 2:
                sums[parts[1].lstrip("*")] = parts[0]
if not sums:
    print("no SHA256SUMS file next to the artifacts"); sys.exit(1)
if not files:
    print("no artifacts found for this platform"); sys.exit(1)
bad = 0
for f in files:
    n = os.path.basename(f)
    if not os.path.isfile(f):
        print("MISSING artifact", n); bad += 1; continue
    got = hashlib.sha256(open(f, "rb").read()).hexdigest()
    if sums.get(n) != got:
        print("MISMATCH", n, "listed", sums.get(n), "actual", got); bad += 1
    else:
        print("OK", n, got)
sys.exit(1 if bad else 0)
PYEOF
}
step checksums s_checksums

# ── step: provenance ─────────────────────────────────────────────────────
s_provenance() {
    if [ -z "$attest_repo" ]; then why "no --attest-repo given (offline or operator run); verify with gh attestation verify manually"; return 77; fi
    command -v gh >/dev/null 2>&1 || { why "gh CLI not available"; return 77; }
    local primary; primary=$(printf '%s\n' $artifact_files | head -1)
    local o rc; o=$(gh attestation verify "$primary" --repo "$attest_repo" 2>&1); rc=$?
    echo "$o"
    if [ $rc -eq 0 ]; then why "attestation verified for $(basename "$primary")"; return 0; fi
    if printf '%s' "$o" | grep -qi 'no attestations found\|not found\|404'; then
        why "no attestation exists for this artifact yet: provenance is created by the publish job after qualification passes and verified there"
        return 77
    fi
    return 1
}
step provenance s_provenance

# ── per-OS install / uninstall primitives ────────────────────────────────
current_version() { "$gaunt" --version 2>&1; }

install_pkg() { # $1 = artifact path
    case "$os" in
        linux) sudo_ apt-get install -y -q "$1" ;;
        macos) sudo_ installer -pkg "$1" -target / ;;
        windows)
            QS_EXE=$(cygpath -w "$1") pwsh -NoProfile -Command '$p = Start-Process -FilePath $env:QS_EXE -ArgumentList "/VERYSILENT","/SUPPRESSMSGBOXES","/CURRENTUSER" -Wait -PassThru; exit $p.ExitCode' ;;
    esac
}
uninstall_pkg() {
    case "$os" in
        linux) sudo_ apt-get remove -y -q gauntlet ;;
        macos)
            sudo_ rm -f /usr/local/bin/gauntlet /usr/local/bin/gauntlet-workbench
            sudo_ rm -rf /usr/local/share/doc/gauntlet
            sudo_ pkgutil --forget ai.godspeed.gauntlet ;;
        windows)
            local u; u=$(ls "$appdir_u"/unins*.exe 2>/dev/null | head -1)
            [ -n "$u" ] || { echo "uninstaller not found in $appdir_u"; return 1; }
            QS_EXE=$(cygpath -w "$u") pwsh -NoProfile -Command '$p = Start-Process -FilePath $env:QS_EXE -ArgumentList "/VERYSILENT","/SUPPRESSMSGBOXES" -Wait -PassThru; exit $p.ExitCode'
            local i=0; while [ -e "$gaunt" ] && [ $i -lt 40 ]; do sleep 0.5; i=$((i + 1)); done ;;
    esac
}
prev_artifact() {
    [ -n "$prev_dir" ] && [ -n "$prev_version" ] || return 1
    case "$os" in
        linux) ls "$prev_dir"/gauntlet_"${prev_version}"_"${arch}".deb 2>/dev/null | head -1 ;;
        macos) ls "$prev_dir"/gauntlet-"${prev_version}"-macos-*.pkg 2>/dev/null | head -1 ;;
        windows) ls "$prev_dir"/gauntlet-"${prev_version}"-x86_64-pc-windows-msvc-setup.exe 2>/dev/null | head -1 ;;
    esac
}

# ── step: tarball / zip payload (no installer) ───────────────────────────
s_tarball_install() {
    local d="$work/payload"; mkdir -p "$d"
    case "$os" in
        linux)
            tar -xzf "$tarball" -C "$d" || return 1
            bin="$d/gauntlet-${version}-${triple}/gauntlet" ;;
        windows)
            QS_ZIP=$(cygpath -w "$zipf") QS_DST=$(cygpath -w "$d") pwsh -NoProfile -Command 'Expand-Archive -LiteralPath $env:QS_ZIP -DestinationPath $env:QS_DST -Force' || return 1
            bin=$(find "$d" -name gauntlet.exe | head -1) ;;
        macos) why "macOS ships a .pkg/.dmg only (no tarball); the pkg is exercised in the install step"; return 77 ;;
    esac
    [ -n "$bin" ] && [ -x "$bin" ] || { why "gauntlet binary not found in the payload"; return 1; }
    if [ "$mode" = tarball ]; then
        cp "$bin" "$prefix/bin/gauntlet" || return 1
        [ -x "$(dirname "$bin")/gauntlet-workbench" ] && cp "$(dirname "$bin")/gauntlet-workbench" "$prefix/bin/gauntlet-workbench"
        bin="$prefix/bin/gauntlet"
    fi
    got=$("$bin" --version 2>&1)
    [ "$got" = "gauntlet ${version}" ] || { why "payload --version printed '$got'"; return 1; }
    why "payload runs: $got"
}
step tarball_install s_tarball_install

# ── step: upgrade from the previous release, then install the candidate ──
prev_installed=0
s_install_previous() {
    if [ "$mode" = tarball ]; then why ".deb install/upgrade need sudo, which this host does not allow unattended (tarball mode)"; return 77; fi
    local p; p=$(prev_artifact) || true
    if [ -z "$p" ]; then
        if [ -z "$prev_version" ]; then why "no previous released version exists to upgrade from"; else why "previous release $prev_version has no ${os}/${arch} installer asset"; fi
        return 77
    fi
    install_pkg "$p" || return 1
    got=$(current_version)
    [ "$got" = "gauntlet ${prev_version}" ] || { why "previous install reports '$got'"; return 1; }
    prev_installed=1
    echo "previous $prev_version installed"
}
# Run in the current shell (not a subshell) so prev_installed survives.
: > "$reasonf"; echo "qualify: ---- install_previous ----"
s_install_previous > "$work/install_previous.log" 2>&1; prev_rc=$?
tail -n 15 "$work/install_previous.log"
prev_reason=$(cat "$reasonf" 2>/dev/null)
[ $prev_rc -eq 0 ] && prev_installed=1 || prev_installed=0
[ $prev_rc -ne 0 ] && [ $prev_rc -ne 77 ] && rec install_previous FAIL "$prev_reason"

s_install() {
    if [ "$mode" = tarball ]; then why ".deb install needs sudo, which this host does not allow unattended; the tarball was installed into a user prefix in tarball_install"; return 77; fi
    local before=""
    [ "$prev_installed" = 1 ] && before=$(current_version)
    local a; a=$(printf '%s\n' $artifact_files | head -1)
    [ "$os" = windows ] && a="$setup"
    [ "$os" = linux ] && a="$deb"
    install_pkg "$a" || return 1
    [ -x "$gaunt" ] || { why "$gaunt missing after install"; return 1; }
    got=$(current_version)
    [ "$got" = "gauntlet ${version}" ] || { why "installed --version printed '$got'"; return 1; }
    if [ "$os" = linux ]; then dpkg -s gauntlet | grep -q '^Status: install ok installed' || return 1; fi
    if [ "$os" = windows ]; then
        pwsh -NoProfile -Command '$u=[Environment]::GetEnvironmentVariable("Path","User"); if (($u -split ";") -contains "$env:LOCALAPPDATA\Programs\Gauntlet") { exit 0 } else { exit 1 }' \
            || { why "user PATH lacks the install dir"; return 1; }
    fi
    why "installed $got"
    printf '%s' "$before" > "$work/before_upgrade"
}
step install s_install

s_upgrade() {
    if [ "$mode" = tarball ]; then why ".deb upgrade needs sudo, which this host does not allow unattended (tarball mode)"; return 77; fi
    if [ "$prev_installed" != 1 ]; then
        why "${prev_reason:-no previous released version exists to upgrade from}"; return 77
    fi
    got=$(current_version)
    [ "$got" = "gauntlet ${version}" ] || { why "after upgrade --version printed '$got'"; return 1; }
    why "upgraded $(cat "$work/before_upgrade") -> $got"
}
step upgrade s_upgrade

# ── step: startup and core-function smoke ────────────────────────────────
fixture="$repo_root/tests/fixtures/target"
s_smoke() {
    local wb=""
    [ -x "$wbench" ] && wb="$wbench"
    if [ "$os" = macos ] && [ "$(uname -m)" != arm64 ]; then
        wb=""; echo "workbench is arm64-only on macOS; not run on this Intel host"
    fi
    [ "$("$gaunt" --version)" = "gauntlet ${version}" ] || return 1
    local proj="$work/doctor-project"; rm -rf "$proj"; cp -a "$fixture" "$proj"
    ( cd "$proj" && "$gaunt" doctor ) || { why "gauntlet doctor failed in the fixture project"; return 1; }
    ( cd "$work" && "$gaunt" agent doctor ) || { why "gauntlet agent doctor reported a broken adapter"; return 1; }
    export GAUNTLET_SMOKE_PROJECT="$fixture" GAUNTLET_EXPECTED_VERSION="$version"
    case "$os" in
        linux) sh "$repo_root/packaging/smoke-gauntlet.sh" "$gaunt" $wb "$version" ;;
        macos) sh "$repo_root/packaging/smoke-macos.sh" "$gaunt" $wb "$version" ;;
        windows)
            local g; g=$(cygpath -w "$gaunt")
            if [ -n "$wb" ]; then
                pwsh -NoProfile -File "$(cygpath -w "$repo_root/packaging/smoke-windows.ps1")" -Gauntlet "$g" -ExpectedVersion "$version" -Workbench "$(cygpath -w "$wb")"
            else
                pwsh -NoProfile -File "$(cygpath -w "$repo_root/packaging/smoke-windows.ps1")" -Gauntlet "$g" -ExpectedVersion "$version"
            fi ;;
    esac || { why "packaging smoke script failed"; return 1; }
    why "--version, doctor (in fixture project), agent doctor, packaging smoke"
}
step smoke s_smoke

# ── step: one governed run with the mock adapter ─────────────────────────
s_governed_run() {
    if [ -z "${GAUNTLET_DATABASE_URL:-}" ]; then why "GAUNTLET_DATABASE_URL is not set: no PostgreSQL provisioned on this host (the state store is required for run/status)"; return 77; fi
    local proj="$data/project"; rm -rf "$proj"; cp -a "$fixture" "$proj"; chmod 0755 "$proj/scripts/check.sh"
    mkdir -p "$data/state" "$data/evidence" "$data/home"
    local tool="$proj/scripts/check.sh" st="$data/state" ev="$data/evidence"
    if [ "$os" = windows ]; then
        local b; b=$(cygpath -w "$(command -v bash)")
        printf '@echo off\r\n"%s" "%%~dp0check.sh" %%*\r\n' "$b" > "$proj/scripts/check.cmd"
        tool=$(cygpath -w "$proj/scripts/check.cmd"); st=$(cygpath -w "$st"); ev=$(cygpath -w "$ev")
    fi
    export GAUNTLET_STATE_DIR="$st" GAUNTLET_ARTIFACT_DIR="$ev" GAUNTLET_MOCK_SCENARIO=e2e-happy \
        GAUNTLET_ID_SEED=native-qualification GAUNTLET_CLOCK=deterministic GAUNTLET_STALL_TIMEOUT_MS=60000 \
        GAUNTLET_RETRY_BASE_MS=100 GAUNTLET_RETRY_MAX_MS=2000 GAUNTLET_MAX_ATTEMPTS_PER_UNIT=3 \
        GAUNTLET_TOOL_TEST="$tool" HOME="$data/home"
    ( cd "$proj" && "$gaunt" run "native release qualification: mock adapter loop" ) > "$work/run.log" 2>&1 \
        || { tail -n 30 "$work/run.log"; why "gauntlet run (mock e2e-happy) exited nonzero"; return 1; }
    local rid; rid=$(sed -n 's/^run: run_id=//p' "$work/run.log" | tail -1 | tr -d '\r')
    [ -n "$rid" ] || { tail -n 30 "$work/run.log"; why "run printed no run_id"; return 1; }
    ( cd "$proj" && "$gaunt" status --run "$rid" ) > "$work/status.log" 2>&1 || { why "gauntlet status failed"; return 1; }
    grep -Eq 'settled=[1-9][0-9]*' "$work/status.log" || { tail -n 20 "$work/status.log"; why "status shows no settled units"; return 1; }
    why "mock adapter run $rid settled"
}
step governed_run s_governed_run

# ── step: uninstall, no binaries remain, user data handled as documented ─
s_uninstall() {
    if [ "$mode" = tarball ]; then
        rm -rf "$prefix/bin"
        [ ! -e "$gaunt" ] && [ ! -e "$wbench" ] || { why "prefix binaries remain"; return 1; }
        if [ -d "$data/project" ]; then
            [ -d "$data/state" ] && [ -d "$data/evidence" ] || { why "run state or evidence was deleted"; return 1; }
            why "prefix removed; project, run state and evidence kept (system packages untouched)"
        else
            why "prefix removed (no governed-run user data existed to preserve)"
        fi
        return 0
    fi
    uninstall_pkg || return 1
    [ ! -e "$gaunt" ] || { why "$gaunt still present"; return 1; }
    [ ! -e "$wbench" ] || { why "$wbench still present"; return 1; }
    if [ "$os" != windows ] && command -v gauntlet >/dev/null 2>&1; then why "gauntlet still resolves on PATH: $(command -v gauntlet)"; return 1; fi
    if [ "$os" = linux ]; then
        ! dpkg -s gauntlet 2>/dev/null | grep -q '^Status: install ok installed' || { why "dpkg still lists gauntlet installed"; return 1; }
    fi
    if [ "$os" = macos ]; then
        ! pkgutil --pkg-info ai.godspeed.gauntlet >/dev/null 2>&1 || { why "pkgutil still knows the package"; return 1; }
        [ ! -e /usr/local/share/doc/gauntlet ] || { why "doc dir remains"; return 1; }
    fi
    if [ "$os" = windows ]; then
        pwsh -NoProfile -Command '$u=[Environment]::GetEnvironmentVariable("Path","User"); if (($u -split ";") -contains "$env:LOCALAPPDATA\Programs\Gauntlet") { exit 1 } else { exit 0 }' \
            || { why "user PATH still names the install dir"; return 1; }
    fi
    # docs: uninstall never deletes project data or run state/evidence
    if [ -d "$data/project" ]; then
        [ -d "$data/state" ] && [ -d "$data/evidence" ] || { why "run state or evidence was deleted by uninstall"; return 1; }
        why "binaries removed; project, run state and evidence kept as documented"
    else
        why "binaries removed (no governed-run user data existed to preserve)"
    fi
}
step uninstall s_uninstall

# ── record ───────────────────────────────────────────────────────────────
# Make sure the step list always names install_previous only when it failed
"$PY" "$here/qualify.py" record "$steps" "$out" \
    "platform=$platform" "evidence=$evidence" "version=$version" "commit=$commit" \
    "runner_image=$runner_image" "os=$os" "arch=$arch" "detail=$detail" \
    "artifacts=$artifacts_meta" "mode=$mode" || exit 2
cat "$out"
if grep -q '"status": "NOT QUALIFIED"' "$out"; then exit 1; fi
exit 0
