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
#   3. `doctor` in an env-only dir (no GAUNTLET.md anywhere above) FAILS
#      CLOSED, by design: the §8.5 reference check fails with no project /
#      reference manifest (CI run 40949652982 + adversarial F1 proved the
#      previous exit-0 assertion could never pass; the product is correct
#      and is never weakened to pass a smoke). The contract asserted here:
#      a typed nonzero exit, the summary naming the failure
#      (`exit: FAIL ...`), the `codemode (optional):` line, the
#      `report: <path>` line pointing at
#      `$GAUNTLET_REPORT_DIR/doctor.json`, and that file written non-empty.
#   4. `gauntlet init` scaffolds a fresh project, and a second init refuses
#      to overwrite it. (The comments-only scaffold is deliberately NOT
#      doctor-clean, so the in-project leg below does not use it.)
#   5. `doctor` exits 0 (`exit: OK`, codemode line, `report: <path>` line,
#      non-empty report file) in a COPY of GAUNTLET_SMOKE_PROJECT — a real,
#      reference-manifested project. GAUNTLET_SMOKE_PROJECT is a REQUIRED
#      caller obligation (CI passes the checkout's tests/fixtures/target);
#      the script refuses with typed exit-2 errors rather than skip the leg
#      silently when it is unset, not a readable directory, or lacking
#      GAUNTLET.md.
#   6. workbench leg (only when a workbench binary is passed): a REAL PTY at
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

GT=$(CDPATH='' cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
if [ "$#" -eq 3 ]; then
    WB=$(CDPATH='' cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
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

# The in-project doctor leg's ruler: a doctor-clean project directory. The
# smoke runs outside any checkout and cannot discover one itself, so the
# caller supplies it (a checkout's tests/fixtures/target). Refusing here —
# like a missing binary above — keeps the leg from ever being skipped
# silently.
smoke_project_input=${GAUNTLET_SMOKE_PROJECT:-}
if [ -z "$smoke_project_input" ]; then
    echo "error: no smoke project: export GAUNTLET_SMOKE_PROJECT to a doctor-clean project directory (a checkout's tests/fixtures/target)" >&2
    exit 2
fi
smoke_project=$(CDPATH='' cd -- "$smoke_project_input" 2>/dev/null && pwd) || {
    echo "error: GAUNTLET_SMOKE_PROJECT is not a readable directory: $smoke_project_input" >&2
    exit 2
}
if [ ! -f "$smoke_project/GAUNTLET.md" ]; then
    echo "error: GAUNTLET_SMOKE_PROJECT carries no GAUNTLET.md (want a doctor-clean project like tests/fixtures/target): $smoke_project" >&2
    exit 2
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
dump() {
    # $1 = label, $2 = file; last ~1500 chars to stderr
    echo "smoke: ---- $1 (tail) ----" >&2
    tail -c 1500 "$2" >&2 2>/dev/null || true
    echo "smoke: ---- end $1 ----" >&2
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

# ── 3. doctor in an env-only dir FAILS CLOSED (the pinned contract) ──────
# With no GAUNTLET.md anywhere above the cwd, the §8.5 reference check
# fails and doctor exits nonzero — fail-closed by design (adversarial
# review + CI run 40949652982 proved the previous exit-0 assertion could
# never pass; the product is correct). The contract asserted here, verified
# against the release binary: typed nonzero exit, the human summary naming
# the failed check, the codemode line, and the machine report still written
# under GAUNTLET_REPORT_DIR (OD-7/OD-14).
mkdir -p "$work/envdir"
doctor_code=0
( cd "$work/envdir" && "$GT" doctor ) \
    > "$work/doctor.out" 2> "$work/doctor.err" || doctor_code=$?
if [ "$doctor_code" -eq 0 ]; then
    dump "doctor stdout" "$work/doctor.out"
    dump "doctor stderr" "$work/doctor.err"
    fail "doctor exited 0 in an env-only dir; the fail-closed contract wants a typed nonzero exit"
fi
grep -q '^exit: FAIL ' "$work/doctor.out" \
    || fail "env-only doctor output lacks the typed 'exit: FAIL' summary line"
grep -q '^codemode (optional): ' "$work/doctor.out" \
    || fail "env-only doctor output lacks the 'codemode (optional):' line"
report_path=$(sed -n 's/^report: //p' "$work/doctor.out" | tail -1)
test -n "$report_path" \
    || fail "env-only doctor output lacks the 'report: <path>' line"
test "$report_path" = "$GAUNTLET_REPORT_DIR/doctor.json" \
    || fail "env-only doctor wrote its report at '$report_path', want $GAUNTLET_REPORT_DIR/doctor.json"
test -s "$report_path" \
    || fail "env-only doctor report file missing or empty: $report_path"
pass "doctor fails closed in an env-only dir (exit $doctor_code, report at $report_path)"

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

# ── 5. doctor exits 0 in a real, reference-manifested project ────────────
# The ruler is a COPY of GAUNTLET_SMOKE_PROJECT (the original is never
# touched): its GAUNTLET.md + reference manifest/answer key form the
# smallest doctor-clean project. An init scaffold does NOT suffice — the
# comments-only template configures no reference manifest, so doctor fails
# closed there by design (verified empirically against the release binary).
cp -a "$smoke_project" "$work/project-check"
if ! ( cd "$work/project-check" && "$GT" doctor ) \
    > "$work/doctor-project.out" 2> "$work/doctor-project.err"; then
    dump "doctor-project stdout" "$work/doctor-project.out"
    dump "doctor-project stderr" "$work/doctor-project.err"
    fail "doctor exited nonzero in the GAUNTLET_SMOKE_PROJECT copy"
fi
grep -q '^exit: OK' "$work/doctor-project.out" \
    || fail "in-project doctor output lacks the 'exit: OK' summary line"
grep -q '^codemode (optional): ' "$work/doctor-project.out" \
    || fail "in-project doctor output lacks the 'codemode (optional):' line"
project_report=$(sed -n 's/^report: //p' "$work/doctor-project.out" | tail -1)
test -n "$project_report" \
    || fail "in-project doctor output lacks the 'report: <path>' line"
test -s "$project_report" \
    || fail "in-project doctor report file missing or empty: $project_report"
pass "doctor exit 0 in the copied smoke project, report at $project_report"

# ── 6. workbench leg: real PTY 120x40, markers, clean 'q' exit ──────────
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
