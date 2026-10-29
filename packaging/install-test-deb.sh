#!/bin/sh
# Full acceptance cycle for a built gauntlet .deb, on a Debian/Ubuntu runner
# with sudo: install -> package-content assertions -> the out-of-checkout
# smoke -> the real functional leg against a caller-provisioned PostgreSQL
# (mock-runner loop, serve + curl, workbench connect) -> clean removal ->
# gone assertions. Every assertion is checked; a failure fails the script.
#
# Usage: packaging/install-test-deb.sh <deb-path> <work-root>
#
# Caller obligations (the script provisions NOTHING itself):
#   GAUNTLET_DATABASE_URL  exported and pointing at a REACHABLE PostgreSQL
#                          server (the state store is required by run/status/
#                          inspect/verify/export/serve)
#   GAUNTLET_REPO_ROOT     the gauntlet checkout (tests/fixtures/target, the
#                          smoke script)
#   sudo                   working (dpkg/apt operations)
# The work-root is created if missing and used for the project copy, run
# state, logs, and the workbench cwd; nothing outside it (or the package's
# own /usr paths) is written.
# The sudo'd dpkg/apt redirects are intentional: the invoking user (not root)
# must own the log files so the failure dumps below can read them.
# shellcheck disable=SC2024
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <deb-path> <work-root>" >&2
    exit 2
fi

fail() {
    echo "install-test: FAIL $1" >&2
    exit 1
}
pass() { echo "install-test: PASS $1"; }
dump() {
    echo "install-test: ---- $1 (tail) ----" >&2
    tail -c 2000 "$2" >&2 2>/dev/null || true
    echo "install-test: ---- end $1 ----" >&2
}

deb_dir=$(CDPATH='' cd -- "$(dirname -- "$1")" && pwd)
deb="$deb_dir/$(basename -- "$1")"
workroot=$2
[ -f "$deb" ] || fail "deb not found: $deb"
mkdir -p "$workroot"
workroot=$(CDPATH='' cd -- "$workroot" && pwd)

[ -n "${GAUNTLET_REPO_ROOT:-}" ] || fail "GAUNTLET_REPO_ROOT must point at the gauntlet checkout (fixture + smoke script)"
repo_root=$(CDPATH='' cd -- "$GAUNTLET_REPO_ROOT" && pwd)
[ -f "$repo_root/tests/fixtures/target/GAUNTLET.md" ] || fail "fixture project missing: $repo_root/tests/fixtures/target/GAUNTLET.md"
[ -f "$repo_root/packaging/smoke-gauntlet.sh" ] || fail "smoke script missing: $repo_root/packaging/smoke-gauntlet.sh"

[ -n "${GAUNTLET_DATABASE_URL:-}" ] || fail "GAUNTLET_DATABASE_URL is not set — the caller must provision PostgreSQL and export it; this script provisions nothing"
python3 - "$GAUNTLET_DATABASE_URL" <<'PY' || fail "GAUNTLET_DATABASE_URL does not parse as a postgres URL"
import sys
from urllib.parse import urlsplit
url = urlsplit(sys.argv[1])
assert url.scheme in ("postgres", "postgresql"), url.scheme
assert url.hostname, "no host in %s" % sys.argv[1]
PY
if ! python3 - "$GAUNTLET_DATABASE_URL" <<'PY'
import socket
import sys
from urllib.parse import urlsplit
url = urlsplit(sys.argv[1])
port = url.port or 5432
try:
    socket.create_connection((url.hostname, port), timeout=5).close()
except OSError as exc:
    sys.stderr.write("install-test: PostgreSQL %s:%s is unreachable: %s\n" % (url.hostname, port, exc))
    raise SystemExit(1)
PY
then
    fail "PostgreSQL is not reachable at GAUNTLET_DATABASE_URL — provision it before this script"
fi

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
sudo -v || fail "sudo is required for dpkg/apt"

echo "install-test: deb $deb"
echo "install-test: work root $workroot"

# ── install ──────────────────────────────────────────────────────────────
if ! sudo dpkg -i "$deb" > "$workroot/dpkg-i.log" 2>&1; then
    dump "dpkg -i log" "$workroot/dpkg-i.log"
    echo "install-test: dpkg -i failed; retrying through apt dependency resolution" >&2
    sudo apt-get update -y >> "$workroot/dpkg-i.log" 2>&1 || true
    if ! sudo apt-get install -y "$deb" >> "$workroot/dpkg-i.log" 2>&1; then
        dump "apt install log" "$workroot/dpkg-i.log"
        fail "the deb did not install"
    fi
fi
pass "dpkg -i installed the package"

# ── package-content assertions ───────────────────────────────────────────
dpkg -L gauntlet > "$workroot/listed.txt"
grep -qx '/usr/bin/gauntlet' "$workroot/listed.txt" || fail "/usr/bin/gauntlet is not in the package file list"
grep -qx '/usr/share/doc/gauntlet/copyright' "$workroot/listed.txt" || fail "/usr/share/doc/gauntlet/copyright is not in the package file list"
grep -qx '/usr/share/doc/gauntlet/README' "$workroot/listed.txt" || fail "/usr/share/doc/gauntlet/README is not in the package file list"
test -x /usr/bin/gauntlet || fail "/usr/bin/gauntlet is not executable"
workbench_installed=""
if grep -qx '/usr/bin/gauntlet-workbench' "$workroot/listed.txt"; then
    workbench_installed=/usr/bin/gauntlet-workbench
    test -x "$workbench_installed" || fail "/usr/bin/gauntlet-workbench is not executable"
fi
world_writable=$(dpkg -L gauntlet | while IFS= read -r path; do
    if [ -f "$path" ]; then
        find "$path" -maxdepth 0 -type f -perm -0002 -print 2>/dev/null || true
    fi
done)
[ -z "$world_writable" ] || fail "unexpected world-writable files in the package: $world_writable"
test -n "$(dpkg-deb --field "$deb" Installed-Size)" || fail "the deb carries no Installed-Size"
pass "package contents: binaries executable, docs present, no world-writable files"

# ── smoke (outside the checkout, against the installed binary) ───────────
version=$(dpkg-deb --field "$deb" Version)
if GAUNTLET_EXPECTED_VERSION="$version" sh "$repo_root/packaging/smoke-gauntlet.sh" \
    /usr/bin/gauntlet ${workbench_installed:+"$workbench_installed"} \
    > "$workroot/smoke.log" 2>&1; then
    pass "smoke-gauntlet.sh against the installed binary(es)"
else
    dump "smoke log" "$workroot/smoke.log"
    fail "smoke-gauntlet.sh failed against the installed binary"
fi

# ── functional leg: a real mock-runner loop in a copied fixture project ──
project="$workroot/project"
rm -rf "$project"
cp -a "$repo_root/tests/fixtures/target" "$project"
chmod 0755 "$project/scripts/check.sh"
mkdir -p "$workroot/state" "$workroot/evidence" "$workroot/home"

# The env mirrors the proven e2e mock harness (crates/gauntlet-cli/tests/
# e2e_full_loop_mock.rs): per-leg state/evidence roots, deterministic clock
# and ids, bounded retries, and the fixture's own check command as the test
# tool binding. GAUNTLET_DATABASE_URL is inherited from the caller. These
# stay exported for the run/status/serve/workbench legs below.
export GAUNTLET_STATE_DIR="$workroot/state"
export GAUNTLET_ARTIFACT_DIR="$workroot/evidence"
export GAUNTLET_MOCK_SCENARIO="e2e-happy"
export GAUNTLET_ID_SEED="native-deb-acceptance"
export GAUNTLET_CLOCK="deterministic"
export GAUNTLET_STALL_TIMEOUT_MS="60000"
export GAUNTLET_RETRY_BASE_MS="100"
export GAUNTLET_RETRY_MAX_MS="2000"
export GAUNTLET_MAX_ATTEMPTS_PER_UNIT="3"
export GAUNTLET_TOOL_TEST="$project/scripts/check.sh"
export HOME="$workroot/home"

if ! (cd "$project" && gauntlet run \
    "native packaging acceptance: mock runner loop") \
    > "$workroot/run.log" 2>&1; then
    dump "run log" "$workroot/run.log"
    fail "gauntlet run (mock e2e-happy loop) exited nonzero"
fi
run_id=$(sed -n 's/^run: run_id=//p' "$workroot/run.log" | tail -1)
[ -n "$run_id" ] || {
    dump "run log" "$workroot/run.log"
    fail "the run printed no 'run: run_id=' line"
}
pass "mock runner loop completed (run_id $run_id)"

if ! (cd "$project" && gauntlet status --run "$run_id") \
    > "$workroot/status.log" 2>&1; then
    dump "status log" "$workroot/status.log"
    fail "gauntlet status --run $run_id exited nonzero"
fi
grep -Eq 'settled=[1-9][0-9]*' "$workroot/status.log" || {
    dump "status log" "$workroot/status.log"
    fail "status shows no settled units — the run did not settle"
}
pass "status records the run settled"

# ── serve leg ────────────────────────────────────────────────────────────
port=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
(cd "$project" && exec env GAUNTLET_API_BIND="127.0.0.1:$port" gauntlet serve) \
    > "$workroot/serve.log" 2>&1 &
serve_pid=$!

cleanup_serve() {
    if kill -0 "$serve_pid" 2>/dev/null; then
        kill -INT "$serve_pid" 2>/dev/null || true
        i=0
        while kill -0 "$serve_pid" 2>/dev/null && [ "$i" -lt 10 ]; do
            sleep 0.5
            i=$((i + 1))
        done
        kill -0 "$serve_pid" 2>/dev/null && kill -TERM "$serve_pid" 2>/dev/null || true
        sleep 1
        kill -0 "$serve_pid" 2>/dev/null && kill -KILL "$serve_pid" 2>/dev/null || true
        wait "$serve_pid" 2>/dev/null || true
    fi
}
trap cleanup_serve EXIT

surface_answered=""
i=0
while [ "$i" -lt 30 ]; do
    if curl -fsS "http://127.0.0.1:$port/api/v1/capabilities" > "$workroot/capabilities.json" 2>/dev/null; then
        surface_answered=1
        break
    fi
    if ! kill -0 "$serve_pid" 2>/dev/null; then
        dump "serve log" "$workroot/serve.log"
        fail "gauntlet serve exited before answering (bind 127.0.0.1:$port)"
    fi
    sleep 1
    i=$((i + 1))
done
[ -n "$surface_answered" ] || {
    dump "serve log" "$workroot/serve.log"
    fail "the §20.2 surface did not answer /api/v1/capabilities within 30s"
}
grep -q '"capabilities"' "$workroot/capabilities.json" || fail "/api/v1/capabilities did not return a capabilities document"
if ! curl -fsS "http://127.0.0.1:$port/api/v1/state" > "$workroot/state.json" 2>/dev/null; then
    dump "serve log" "$workroot/serve.log"
    fail "/api/v1/state did not answer"
fi
pass "serve answered capabilities + state on 127.0.0.1:$port"

# ── workbench connect leg (the operator-surface connection proof) ────────
if [ -n "$workbench_installed" ]; then
    log_offset=$(wc -l < "$workroot/serve.log")
    cat > "$workroot/pty_connect.py" <<'PY'
"""Launch the workbench against the live surface in a REAL PTY 120x40 and
qualify the connection: panel markers render, the client has time to fetch
the surface, then `q` exits 0. Mirrors the helper in smoke-gauntlet.sh
(markers mode) with a 3s dwell before quitting so the projection's fetches
land in the serve access log.

Usage: pty_connect.py <binary> <cwd> — env supplies GAUNTLET_ENDPOINT.
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
DWELL = 3.0
QUIT_TIMEOUT = 15.0


def exitcode_of(status):
    if os.WIFEXITED(status):
        return os.WEXITSTATUS(status)
    if os.WIFSIGNALED(status):
        return 128 + os.WTERMSIG(status)
    return None


def die(msg, output):
    sys.stderr.write("pty: FAIL %s\n" % msg)
    sys.stderr.write("pty: ---- last output (up to 1500 chars) ----\n")
    sys.stderr.write(output[-1500:].decode("utf-8", errors="replace") + "\n")
    sys.stderr.write("pty: ---- end output ----\n")
    raise SystemExit(1)


def main():
    binary, cwd = sys.argv[1], sys.argv[2]
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
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
                try:
                    _, stat = os.waitpid(pid, 0)
                    return stat
                except (ChildProcessError, ProcessLookupError):
                    return None
            output += chunk
        return None

    deadline = time.time() + MARKER_TIMEOUT
    while time.time() < deadline:
        stat = drain(0.2)
        if stat is not None:
            status = stat
            break
        if all(m.encode() in output for m in MARKERS):
            break
    if not all(m.encode() in output for m in MARKERS):
        die("the panel markers did not render against the live surface", output)
    drain(DWELL)
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
        die("the workbench did not quit on 'q'", output)
    code = exitcode_of(status)
    if code != 0:
        die("the workbench exited %s on 'q' (want 0)" % code, output)
    raise SystemExit(0)


if __name__ == "__main__":
    main()
PY
    mkdir -p "$workroot/wb-cwd"
    if ! (cd "$project" && GAUNTLET_ENDPOINT="http://127.0.0.1:$port" HOME="$workroot/home" \
        python3 "$workroot/pty_connect.py" "$workbench_installed" "$workroot/wb-cwd") \
        > "$workroot/wb-connect.log" 2>&1; then
        cat "$workroot/wb-connect.log" >&2
        fail "the installed workbench failed the live-surface PTY leg"
    fi
    # The connection proof: GET lines the workbench itself produced (after
    # the curl legs' offset), against the surface's access log.
    if ! tail -n +"$((log_offset + 1))" "$workroot/serve.log" \
        | grep -Eq 'surface: "GET" "/api/v1/.*" -> 200'; then
        dump "serve log (workbench window)" "$workroot/serve.log"
        fail "the workbench rendered but never issued a GET to the operator surface"
    fi
    pass "installed workbench connected to the operator surface and rendered"
fi

# ── stop serve cleanly ───────────────────────────────────────────────────
cleanup_serve
trap - EXIT
if kill -0 "$serve_pid" 2>/dev/null; then
    fail "gauntlet serve did not stop"
fi
pass "gauntlet serve stopped cleanly"

# ── removal ──────────────────────────────────────────────────────────────
if ! sudo apt-get remove -y gauntlet > "$workroot/remove.log" 2>&1; then
    dump "apt remove log" "$workroot/remove.log"
    if ! sudo dpkg -r gauntlet >> "$workroot/remove.log" 2>&1; then
        dump "dpkg remove log" "$workroot/remove.log"
        fail "the package did not remove"
    fi
fi
if dpkg -s gauntlet > /dev/null 2>&1; then
    sudo dpkg -P gauntlet >> "$workroot/remove.log" 2>&1 || {
        dump "dpkg purge log" "$workroot/remove.log"
        fail "the package did not purge"
    }
fi
if dpkg -s gauntlet > /dev/null 2>&1; then
    fail "dpkg still knows the gauntlet package after remove+purge"
fi
[ ! -e /usr/bin/gauntlet ] || fail "/usr/bin/gauntlet survived removal"
[ ! -e /usr/bin/gauntlet-workbench ] || fail "/usr/bin/gauntlet-workbench survived removal"
[ ! -e /usr/share/doc/gauntlet ] || fail "/usr/share/doc/gauntlet survived removal"
pass "remove+purge: binary, workbench, and docs are gone"

echo "install-test: ALL CHECKS PASSED"
