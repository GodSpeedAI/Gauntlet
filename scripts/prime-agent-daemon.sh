#!/usr/bin/env bash
# scripts/prime-agent-daemon.sh — start/stop the prime-agent daemon, ONLY
# when this recipe set is the one that started it.
#
# The prime-agent runner adapter's `stop` and `list` operations are
# daemon-backed: they talk to a Unix socket at
# /tmp/prime-agent-$UID/daemon.sock. Nothing in `prime-agent`'s own CLI
# starts that daemon implicitly — if it isn't already running, ANY cancel
# (e.g. a stall-timeout-triggered stop) fails with a connection error that
# the gauntlet runner adapter currently treats as fatal (see the
# justfile's demo-up doc comment and the project report for the deeper
# defect in gauntlet-adapter-agent-prime's stop-idempotence matching).
#
# This script is deliberately conservative about ownership: it NEVER stops
# a daemon it did not itself start (an operator's own long-running daemon,
# or one from an unrelated session, is left alone — `ensure` checks socket
# presence FIRST and only writes the "started-by-us" marker when it is the
# one launching the process; `stop` only acts when that marker exists).
#
# Usage:
#   prime-agent-daemon.sh ensure <state_dir>
#   prime-agent-daemon.sh stop <state_dir>
set -u

CMD="${1:?usage: prime-agent-daemon.sh ensure|stop <state_dir>}"
STATE_DIR="${2:?usage: prime-agent-daemon.sh ensure|stop <state_dir>}"
mkdir -p "$STATE_DIR"

SOCK="/tmp/prime-agent-$(id -u)/daemon.sock"
PIDFILE="$STATE_DIR/prime-agent-daemon.pid"
MARKER="$STATE_DIR/prime-agent-daemon.started-by-us"
LOGFILE="$STATE_DIR/prime-agent-daemon.log"

case "$CMD" in
  ensure)
    if [ -S "$SOCK" ]; then
      # A second `ensure` against a daemon WE started must keep ownership, or
      # the matching `stop` would silently leak it. Only a socket we have no
      # live pid for is someone else's.
      if [ -f "$MARKER" ] && [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
        echo "prime-agent-daemon: daemon already up (pid $(cat "$PIDFILE")), started by this recipe set."
        exit 0
      fi
      echo "prime-agent-daemon: socket already present at $SOCK — using the existing daemon, not managed by this recipe."
      rm -f "$MARKER" "$PIDFILE"
      exit 0
    fi
    if ! command -v prime-agent >/dev/null 2>&1; then
      echo "prime-agent-daemon: 'prime-agent' not on PATH — cannot start the daemon (dispatch will fail if runners.default is prime-agent)." >&2
      exit 0
    fi
    echo "prime-agent-daemon: no daemon socket at $SOCK — starting 'prime-agent --mode daemon' (log: $LOGFILE)."
    nohup prime-agent --mode daemon >"$LOGFILE" 2>&1 &
    echo $! >"$PIDFILE"
    touch "$MARKER"
    for _ in $(seq 1 40); do
      [ -S "$SOCK" ] && break
      sleep 0.25
    done
    if [ -S "$SOCK" ]; then
      echo "prime-agent-daemon: daemon is up (pid $(cat "$PIDFILE"))."
    else
      echo "prime-agent-daemon: daemon socket did not appear within 10s — check $LOGFILE." >&2
    fi
    ;;
  stop)
    if [ ! -f "$MARKER" ]; then
      echo "prime-agent-daemon: this recipe did not start the daemon — leaving it running."
      exit 0
    fi
    if [ -f "$PIDFILE" ]; then
      pid="$(cat "$PIDFILE")"
      if kill -0 "$pid" 2>/dev/null; then
        echo "prime-agent-daemon: stopping the daemon this recipe started (pid $pid)..."
        kill "$pid" 2>/dev/null || true
        for _ in $(seq 1 20); do
          kill -0 "$pid" 2>/dev/null || break
          sleep 0.25
        done
        kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
        echo "prime-agent-daemon: stopped."
      else
        echo "prime-agent-daemon: tracked pid $pid already gone."
      fi
      rm -f "$PIDFILE"
    fi
    rm -f "$MARKER"
    ;;
  *)
    echo "prime-agent-daemon: unknown command '$CMD' (expected ensure|stop)" >&2
    exit 2
    ;;
esac
