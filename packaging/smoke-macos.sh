#!/bin/sh
# Functional smoke of INSTALLED (packaged) Gauntlet binaries on macOS.
#
# Usage: packaging/smoke-macos.sh <gauntlet-bin> [workbench-bin] <expected-version>
#
# Runs entirely inside a disposable mktemp root (disposable HOME and
# GAUNTLET_REPORT_DIR, trap-cleaned) so it never touches the calling user's
# data. It exercises real installed behavior, not startup alone:
#
#   1. `--version` prints exactly "gauntlet <expected-version>"
#   2. `--help` exits 0 with non-empty output
#   3. `doctor` exits 0 with its machine report inside GAUNTLET_REPORT_DIR
#      (a disposable env-only dir; doctor does not probe PostgreSQL and none
#      is provided or simulated)
#   4. `gauntlet init` scaffolds a fresh project, and a second init refuses
#      to overwrite it
#   5. workbench leg (only when a workbench binary is passed): a REAL PTY at
#      120x40 via python3(1); requires the NAVIGATOR/PRIMARY/INSPECTOR/
#      COMPOSER markers to render, a 'q' keystroke, and a clean exit.
#      Best-effort-honest: if this leg cannot run in the caller's
#      environment it FAILS LOUDLY unless the caller sets
#      GAUNTLET_SMOKE_SKIP_PTY=1, in which case the skip is printed and
#      recorded — never silent.
set -eu

if [ "$#" -ne 2 ] && [ "$#" -ne 3 ]; then
    echo "usage: $0 <gauntlet-bin> [workbench-bin] <expected-version>" >&2
    exit 2
fi

GT=$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
if [ "$#" -eq 3 ]; then
    WB=$(CDPATH= cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
    VERSION=$3
else
    WB=''
    VERSION=$2
fi

if [ ! -x "$GT" ]; then
    echo "error: not executable: $GT" >&2
    exit 1
fi
if [ -n "$WB" ] && [ ! -x "$WB" ]; then
    echo "error: not executable: $WB" >&2
    exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/gauntlet-smoke.XXXXXX")
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

export HOME="$work/home"
export GAUNTLET_REPORT_DIR="$work/report"
unset GAUNTLET_PROJECT_ROOT GAUNTLET_CONFIG_DIR GAUNTLET_DATABASE_URL \
    GAUNTLET_ENDPOINT GAUNTLET_MOCK_SCENARIO 2>/dev/null || true
mkdir -p "$HOME" "$GAUNTLET_REPORT_DIR"

pass() { echo "smoke: PASS $1"; }
fail() {
    echo "smoke: FAIL $1" >&2
    exit 1
}

echo "smoke: gauntlet binary $GT"
if [ -n "$WB" ]; then
    echo "smoke: workbench binary $WB"
else
    echo "smoke: NOTE no workbench binary passed; the workbench PTY leg is not part of this run"
fi
echo "smoke: disposable root $work"

# ── 1. --version exact match ─────────────────────────────────────────────
if ! got=$("$GT" --version); then
    fail "--version exited non-zero"
fi
test "$got" = "gauntlet $VERSION" || fail "version: got '$got', want 'gauntlet $VERSION'"
pass "version -> $got"

# ── 2. --help ─────────────────────────────────────────────────────────────
"$GT" --help > "$work/help.out" 2>&1 || fail "--help exited non-zero"
test -s "$work/help.out" || fail "--help produced no output"
pass "--help exits 0 with usage output"

# ── 3. doctor in a disposable env-only dir ───────────────────────────────
doctor_cwd="$work/doctor-cwd"
mkdir -p "$doctor_cwd"
( cd "$doctor_cwd" && "$GT" doctor ) > "$work/doctor.out" 2>&1 \
    || fail "doctor exited non-zero in a disposable env-only dir: $(cat "$work/doctor.out")"
test -s "$GAUNTLET_REPORT_DIR/doctor.json" \
    || fail "doctor did not write its machine report under GAUNTLET_REPORT_DIR ($GAUNTLET_REPORT_DIR)"
pass "doctor exits 0; report written inside the disposable GAUNTLET_REPORT_DIR"

# ── 4. gauntlet init scaffolds, and refuses to overwrite ────────────────
project="$work/project"
mkdir -p "$project"
( cd "$project" && "$GT" init ) > "$work/init.out" 2>&1 \
    || fail "init failed in a fresh temp project: $(cat "$work/init.out")"
test -f "$project/GAUNTLET.md" || fail "init did not write GAUNTLET.md"
test -f "$project/.env.example" || fail "init did not write .env.example"
test -d "$project/.gauntlet" || fail "init did not create the .gauntlet directory"
if ( cd "$project" && "$GT" init ) > "$work/init2.out" 2>&1; then
    fail "a second init overwrote an existing project; it must refuse"
fi
pass "init scaffolds a project and refuses to overwrite an existing one"

# ── 5. workbench leg: real PTY 120x40, markers, clean 'q' exit ──────────
if [ -z "$WB" ]; then
    echo "smoke: SKIP workbench PTY leg not requested (no workbench binary passed) — recorded"
elif [ "${GAUNTLET_SMOKE_SKIP_PTY:-0}" = "1" ]; then
    echo "smoke: WARNING GAUNTLET_SMOKE_SKIP_PTY=1 — workbench PTY leg SKIPPED by the caller (recorded)"
else
    if ! command -v python3 > /dev/null 2>&1; then
        fail "the workbench PTY leg requires python3 and none is available; to skip it deliberately set GAUNTLET_SMOKE_SKIP_PTY=1"
    fi
    wb_cwd="$work/workbench-cwd"
    mkdir -p "$wb_cwd" "$work/projection"
    cat > "$work/pty_smoke.py" <<'PY'
import fcntl, os, pty, select, signal, struct, sys, termios, time

RENDER_TIMEOUT = 30.0
EXIT_TIMEOUT = 15.0
MARKERS = ("NAVIGATOR", "PRIMARY", "INSPECTOR", "COMPOSER")

exe = sys.argv[1]

pid, fd = pty.fork()
if pid == 0:
    try:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    except OSError:
        pass
    os.environ["TERM"] = "xterm-256color"
    os.environ["COLUMNS"] = "120"
    os.environ["LINES"] = "40"
    try:
        os.execvp(exe, [exe])
    finally:
        os._exit(127)

def reap():
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass

out = b""
rendered = False
render_deadline = time.monotonic() + RENDER_TIMEOUT
while time.monotonic() < render_deadline:
    try:
        ready, _, _ = select.select([fd], [], [], 0.2)
    except InterruptedError:
        continue
    if fd in ready:
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    if b"COMPOSER" in out:
        rendered = True
        break

if not rendered:
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    reap()
    sys.stderr.write(
        "workbench PTY leg: no COMPOSER marker rendered within %.0fs at 120x40; frame tail:\n"
        % RENDER_TIMEOUT
    )
    sys.stderr.write(out[-2000:].decode("utf-8", "replace") + "\n")
    sys.exit(1)

try:
    os.write(fd, b"q")
except OSError as error:
    sys.stderr.write("workbench PTY leg: could not send 'q': %s\n" % error)
    sys.exit(1)

status = None
exit_deadline = time.monotonic() + EXIT_TIMEOUT
while time.monotonic() < exit_deadline:
    try:
        waited_pid, waited_status = os.waitpid(pid, os.WNOHANG)
    except InterruptedError:
        continue
    if waited_pid == pid:
        status = waited_status
        break
    try:
        ready, _, _ = select.select([fd], [], [], 0.1)
        if fd in ready:
            try:
                out += os.read(fd, 65536)
            except OSError:
                pass
    except InterruptedError:
        continue

if status is None:
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    reap()
    sys.stderr.write("workbench PTY leg: did not exit within %.0fs of 'q'\n" % EXIT_TIMEOUT)
    sys.exit(1)

missing = [m for m in MARKERS if m.encode("utf-8") not in out]
if missing:
    sys.stderr.write(
        "workbench PTY leg: markers missing from the rendered frame: %s; frame tail:\n"
        % ", ".join(missing)
    )
    sys.stderr.write(out[-2000:].decode("utf-8", "replace") + "\n")
    sys.exit(1)

if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
    if os.WIFEXITED(status):
        detail = "exit %d" % os.WEXITSTATUS(status)
    else:
        detail = "signal %d" % os.WTERMSIG(status)
    sys.stderr.write("workbench PTY leg: did not exit cleanly on 'q' (%s)\n" % detail)
    sys.exit(1)

print("markers %s rendered at 120x40; clean exit on 'q'" % "/".join(MARKERS))
sys.exit(0)
PY
    # The workbench needs a writable cwd for its projection store and
    # honors GAUNTLET_WORKBENCH_GAUNTLET_BINARY; both point inside the
    # disposable root. It renders (disconnected) without a live endpoint,
    # exactly like the release-manifest launch qualification.
    (
        cd "$wb_cwd" && \
        GAUNTLET_WORKBENCH_GAUNTLET_BINARY="$GT" \
        GAUNTLET_WORKBENCH_PROJECTION_STORE="$work/projection" \
        python3 "$work/pty_smoke.py" "$WB"
    ) || fail "workbench PTY leg failed (see diagnostics above)"
    pass "workbench PTY leg: rendered and exited cleanly on 'q'"
fi

echo "smoke: ALL CHECKS PASSED"
