#!/usr/bin/env bash
# scripts/serve-supervisor.sh — retry `gauntlet serve` until a run exists.
#
# `gauntlet serve` refuses to open until at least one real run has been
# composed in its project (durable state must exist — nothing is
# fabricated). For a real project (as opposed to the mock scratch project
# `just tui-dev-up` seeds by default) we never compose a fake run just to
# satisfy that gate — the operator composes the first real run themselves,
# from inside the already-launched workbench TUI (or the CLI). So this
# script sits in a retry loop, attempting `gauntlet serve` every
# RETRY_INTERVAL seconds, until one attempt actually stays up and answers
# — at which point it writes that process's pid to PIDFILE and exits,
# leaving `gauntlet serve` running in the background. It also exits
# (without ever having started serve) if STOPFILE appears, so a teardown
# recipe can cancel a supervisor that's still retrying.
#
# Usage: serve-supervisor.sh <gauntlet_bin> <project_dir> <bind> <pidfile> <logfile> <stopfile>
set -u

GAUNTLET_BIN="$1"
PROJECT_DIR="$2"
BIND="$3"
PIDFILE="$4"
LOGFILE="$5"
STOPFILE="$6"
RETRY_INTERVAL="${SERVE_SUPERVISOR_RETRY_SECONDS:-3}"

probe() {
  curl -fsS -o /dev/null --max-time 1 "http://$BIND/api/v1/capabilities" 2>/dev/null
}

echo "serve-supervisor: watching $PROJECT_DIR — retrying 'gauntlet serve' on http://$BIND every ${RETRY_INTERVAL}s until a run exists" >>"$LOGFILE"

while true; do
  if [ -f "$STOPFILE" ]; then
    echo "serve-supervisor: stop requested before serve ever came up — exiting." >>"$LOGFILE"
    rm -f "$STOPFILE"
    exit 0
  fi

  (
    cd "$PROJECT_DIR" || exit 1
    GAUNTLET_API_BIND="$BIND" exec "$GAUNTLET_BIN" serve
  ) >>"$LOGFILE" 2>&1 &
  attempt_pid=$!

  ready=0
  for _ in $(seq 1 20); do
    if probe; then
      ready=1
      break
    fi
    if ! kill -0 "$attempt_pid" 2>/dev/null; then
      break
    fi
    sleep 0.25
  done

  if [ "$ready" -eq 1 ]; then
    echo "$attempt_pid" >"$PIDFILE"
    echo "serve-supervisor: 'gauntlet serve' (pid $attempt_pid) is answering at http://$BIND — a run exists, supervisor done." >>"$LOGFILE"
    exit 0
  fi

  kill "$attempt_pid" 2>/dev/null || true
  wait "$attempt_pid" 2>/dev/null || true
  sleep "$RETRY_INTERVAL"
done
