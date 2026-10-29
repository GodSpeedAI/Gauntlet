#!/bin/sh
# Functional smoke of a PACKAGED (installed or staged) Gauntlet binary, run
# entirely OUTSIDE the repository checkout: disposable HOME, XDG and report
# roots under one mktemp dir, cleaned up on exit. The script never writes
# outside its own temp roots.
#
# Usage: packaging/smoke-gauntlet.sh <gauntlet-binary> [workbench-binary] [expected-version]
#
# The expected version comes from argument 3 or GAUNTLET_EXPECTED_VERSION
# (the script runs outside the checkout, so it cannot read the repo's
# Cargo.toml; the caller — CI or install-test-deb.sh — supplies it). With
# neither, the script refuses.
#
# Assertions (every one checked; non-zero exit on the first failure):
#   1. `--version` prints exactly `gauntlet <expected-version>`
#   2. `--help` exits 0 and prints the usage block
#   3. `doctor` exits 0 in an env-only dir (no GAUNTLET.md anywhere above),
#      prints its `codemode (optional):` line, and its `report:` file exists
#   4. `gauntlet init` scaffolds a fresh project dir (syntax probed via
#      `init --help` first: init refuses arguments and prints its usage)
#   5. when a workbench binary was passed:
#        a. REAL PTY 120x40 (TERM=xterm-256color): the NAVIGATOR / PRIMARY /
#           INSPECTOR / COMPOSER panel markers render within 30s, then `q`
#           quits with exit 0
#        b. REAL PTY 80x24: clean start (no startup crash) and clean `q`
#          exit — compact layout is not over-asserted
#
# The workbench legs need python3 (the runner has it); the PTY size is set
# explicitly, since a CI context has no controlling terminal to inherit one
# from. Marker/quit semantics mirror the proven launch qualification in
# workbench/src/release-manifest.ts.
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 3 ]; then
    echo "usage: $0 <gauntlet-binary> [workbench-binary] [expected-version]" >&2
    exit 2
fi

gauntlet_bin=$1
workbench_bin=${2:-}
expected=${3:-${GAUNTLET_EXPECTED_VERSION:-}}

if [ -z "$expected" ]; then
    echo "error: no expected version: pass argument 3 or export GAUNTLET_EXPECTED_VERSION" >&2
    exit 2
fi

# Absolute paths FIRST: the script changes directory into its temp roots.
# A bare name (found on PATH) is resolved through command -v; a slashed path
# is resolved relative to the caller's cwd.
resolve_binary() {
    case "$1" in
        */*)
            dir=$(CDPATH='' cd -- "$(dirname -- "$1")" && pwd)
            printf '%s/%s\n' "$dir" "$(basename -- "$1")"
            ;;
        *)
            command -v "$1" || {
                echo "error: binary not found on PATH: $1" >&2
                exit 1
            }
            ;;
    esac
}
gauntlet_bin=$(resolve_binary "$gauntlet_bin")
if [ -n "$workbench_bin" ]; then
    workbench_bin=$(resolve_binary "$workbench_bin")
fi

if [ ! -x "$gauntlet_bin" ]; then
    echo "error: not executable: $gauntlet_bin" >&2
    exit 1
fi
if [ -n "$workbench_bin" ] && [ ! -x "$workbench_bin" ]; then
    echo "error: not executable: $workbench_bin" >&2
    exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/gauntlet-smoke.XXXXXX")
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

# Disposable identity and state roots; the nothing-configured doctor leg
# requires the caller's Gauntlet environment to stay out of the way.
export HOME="$work/home"
export XDG_CACHE_HOME="$work/xdg-cache"
export XDG_DATA_HOME="$work/xdg-data"
export XDG_STATE_HOME="$work/xdg-state"
export GAUNTLET_REPORT_DIR="$work/reports"
mkdir -p "$HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$GAUNTLET_REPORT_DIR"
unset GAUNTLET_DATABASE_URL GAUNTLET_API_BIND GAUNTLET_API_TOKEN GAUNTLET_SESSION_TOKEN \
    GAUNTLET_ENDPOINT GAUNTLET_WORKBENCH_ENDPOINT GAUNTLET_PROJECT_ROOT GAUNTLET_MOCK_SCENARIO \
    GAUNTLET_TOOL_TEST GAUNTLET_STATE_DIR GAUNTLET_ARTIFACT_DIR 2>/dev/null || true

pass() { echo "smoke: PASS $1"; }
dump() {
    # $1 = label, $2 = file; last ~1500 chars to stderr
    echo "smoke: ---- $1 (tail) ----" >&2
    tail -c 1500 "$2" >&2 2>/dev/null || true
    echo "smoke: ---- end $1 ----" >&2
}
fail() {
    echo "smoke: FAIL $1" >&2
    exit 1
}

echo "smoke: binary $gauntlet_bin"
echo "smoke: fresh roots under $work"

# ── 1. exact version ─────────────────────────────────────────────────────
if ! got=$("$gauntlet_bin" --version 2> "$work/version.err"); then
    dump "version stderr" "$work/version.err"
    fail "--version exited nonzero"
fi
test "$got" = "gauntlet $expected" || fail "version: got '$got', want 'gauntlet $expected'"
pass "version -> $got"

# ── 2. help ──────────────────────────────────────────────────────────────
if ! "$gauntlet_bin" --help > "$work/help.txt" 2> "$work/help.err"; then
    dump "help stderr" "$work/help.err"
    fail "--help exited nonzero"
fi
grep -q '^usage:' "$work/help.txt" || fail "--help printed no usage block"
pass "--help exits 0 with usage"

# ── 3. doctor in an env-only dir ─────────────────────────────────────────
mkdir -p "$work/envdir"
if ! (cd "$work/envdir" && "$gauntlet_bin" doctor) > "$work/doctor.out" 2> "$work/doctor.err"; then
    dump "doctor stdout" "$work/doctor.out"
    dump "doctor stderr" "$work/doctor.err"
    fail "doctor exited nonzero in an env-only dir"
fi
grep -q '^codemode (optional): ' "$work/doctor.out" \
    || fail "doctor output lacks the 'codemode (optional):' line"
report_path=$(sed -n 's/^report: //p' "$work/doctor.out" | tail -1)
test -n "$report_path" || fail "doctor output lacks the 'report: <path>' line"
test -s "$report_path" || fail "doctor report file missing or empty: $report_path"
pass "doctor exit 0 (env-only dir), report at $report_path"

# ── 4. init in a fresh project dir ───────────────────────────────────────
mkdir -p "$work/fresh"
# Syntax probe: init refuses arguments and prints its usage — which is the
# confirmation that plain `init` (no args) is the correct invocation.
set +e
(cd "$work/fresh" && "$gauntlet_bin" init --help) > "$work/init-probe.out" 2>&1
set -e
grep -q 'usage: gauntlet init' "$work/init-probe.out" \
    || fail "init syntax probe: no 'usage: gauntlet init' in the probe output"
if ! (cd "$work/fresh" && "$gauntlet_bin" init) > "$work/init.out" 2> "$work/init.err"; then
    dump "init stderr" "$work/init.err"
    fail "init exited nonzero in a fresh dir"
fi
test -f "$work/fresh/GAUNTLET.md" || fail "init did not scaffold GAUNTLET.md"
test -f "$work/fresh/.env.example" || fail "init did not scaffold .env.example"
test -d "$work/fresh/.gauntlet" || fail "init did not scaffold the .gauntlet directory"
pass "init scaffolded a fresh project (GAUNTLET.md, .env.example, .gauntlet/)"

# ── 5. workbench PTY legs ────────────────────────────────────────────────
if [ -n "$workbench_bin" ]; then
    if ! command -v python3 >/dev/null 2>&1; then
        fail "python3 is required for the workbench PTY legs"
    fi
    cat > "$work/pty_run.py" <<'PY'
"""Run a TUI binary in a REAL PTY of an exact size and qualify its launch.

Usage: pty_run.py <binary> <cols> <rows> <markers|compact> <cwd>

markers: require every marker in NAVIGATOR/PRIMARY/INSPECTOR/COMPOSER within
  30s, then write "q" and require exit 0 within 15s.
compact: require a clean start (alive and rendering after 3s), then write "q"
  and require exit 0 within 15s.

Exit 0 on success. On failure: diagnostics plus the last ~1500 chars of PTY
output on stderr, exit 1.
"""
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

MARKERS = ("NAVIGATOR", "PRIMARY", "INSPECTOR", "COMPOSER")
MARKER_TIMEOUT = 30.0
SETTLE_TIMEOUT = 3.0
QUIT_TIMEOUT = 15.0


def exitcode_of(status):
    if os.WIFEXITED(status):
        return os.WEXITSTATUS(status)
    if os.WIFSIGNALED(status):
        return 128 + os.WTERMSIG(status)
    return None


def die(msg, output):
    sys.stderr.write("pty: FAIL %s\n" % msg)
    tail = output[-1500:].decode("utf-8", errors="replace")
    sys.stderr.write("pty: ---- last output (up to 1500 chars) ----\n%s\n" % tail)
    sys.stderr.write("pty: ---- end output ----\n")
    raise SystemExit(1)


def main():
    binary, cols, rows, mode, cwd = (
        sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5])
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    pid = os.fork()
    if pid == 0:
        os.close(master)
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        if slave > 2:
            os.close(slave)
        os.chdir(cwd)
        env = dict(os.environ)
        env["TERM"] = "xterm-256color"
        env.setdefault("COLUMNS", str(cols))
        env.setdefault("LINES", str(rows))
        try:
            os.execve(binary, [binary], env)
        except OSError as exc:
            os.write(2, ("pty: exec failed: %s\n" % exc).encode())
            os._exit(127)
    os.close(slave)

    output = b""
    status = None

    def drain(seconds):
        nonlocal output
        deadline = time.time() + seconds
        while time.time() < deadline:
            remaining = max(0.0, deadline - time.time())
            try:
                ready, _, _ = select.select([master], [], [], min(0.2, remaining))
            except InterruptedError:
                continue
            if not ready:
                done, stat = os.waitpid(pid, os.WNOHANG)
                if done == pid:
                    return stat
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError as exc:
                if exc.errno != errno.EIO:
                    raise
                chunk = b""
            if not chunk:
                # Child side closed (EOF or EIO): reap the exit immediately
                # so callers never spin out the rest of their deadline.
                try:
                    _, stat = os.waitpid(pid, 0)
                    return stat
                except (ChildProcessError, ProcessLookupError):
                    return None
            output += chunk
        return None

    if mode == "markers":
        deadline = time.time() + MARKER_TIMEOUT
        while time.time() < deadline:
            stat = drain(0.2)
            if stat is not None:
                status = stat
                break
            if all(m.encode() in output for m in MARKERS):
                break
        if not all(m.encode() in output for m in MARKERS):
            die("the panel markers did not render within %.0fs" % MARKER_TIMEOUT, output)
    elif mode == "compact":
        stat = drain(SETTLE_TIMEOUT)
        if stat is not None:
            status = stat
            die("the TUI exited during the compact-start settle window "
                "(exit code %s)" % exitcode_of(status), output)
    else:
        raise SystemExit("pty: unknown mode %r" % mode)

    # Quit through the keyboard — the real operator exit path. The TUI is
    # never killed while qualifying unless it refuses to leave.
    try:
        os.write(master, b"q")
    except OSError:
        pass
    quit_deadline = time.time() + QUIT_TIMEOUT
    while status is None and time.time() < quit_deadline:
        stat = drain(0.2)
        if stat is not None:
            status = stat
    if status is None:
        try:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        except (ProcessLookupError, ChildProcessError):
            pass
        die("the TUI did not quit on 'q' within %.0fs" % QUIT_TIMEOUT, output)
    code = exitcode_of(status)
    if code != 0:
        die("the TUI exited %s on 'q' (want 0)" % code, output)
    raise SystemExit(0)


if __name__ == "__main__":
    main()
PY

    mkdir -p "$work/wb-cwd"
    echo "smoke: workbench $workbench_bin"
    if ! python3 "$work/pty_run.py" "$workbench_bin" 120 40 markers "$work/wb-cwd" \
        > "$work/pty120.log" 2>&1; then
        cat "$work/pty120.log" >&2
        fail "workbench 120x40 PTY leg failed"
    fi
    pass "workbench 120x40: all panel markers rendered, clean q exit 0"
    if ! python3 "$work/pty_run.py" "$workbench_bin" 80 24 compact "$work/wb-cwd" \
        > "$work/pty80.log" 2>&1; then
        cat "$work/pty80.log" >&2
        fail "workbench 80x24 PTY leg failed"
    fi
    pass "workbench 80x24: clean start, clean q exit 0"
fi

echo "smoke: ALL CHECKS PASSED"
