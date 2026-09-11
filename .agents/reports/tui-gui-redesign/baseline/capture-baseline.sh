#!/usr/bin/env bash
# Baseline frame capture for the EXISTING Gauntlet TUI workbench (read-only recon).
# Reproduce: bash .agents/reports/tui-gui-redesign/baseline/capture-baseline.sh
# Populated leg needs a gauntlet surface answering at 127.0.0.1:4000 (e.g. `just tui-dev-up`
# backend half, or an already-running `gauntlet serve` in workbench/.dev/scratch-project).
# Projection store forced to :memory: so the launch writes no .gauntlet/projections.db.
set -u
ROOT=/home/sprime01/projects/gauntlet
OUT=$ROOT/.agents/reports/tui-gui-redesign/baseline
T="tmux -L gxrecon"
ENVCOMMON="env -u GAUNTLET_ENDPOINT -u GAUNTLET_WORKBENCH_PRIME_BINARY -u GAUNTLET_WORKBENCH_PRIME_SOCKET TERM=xterm-256color GAUNTLET_WORKBENCH_PROJECTION_STORE=:memory:"
LAUNCH_POP="cd $ROOT/workbench && $ENVCOMMON GAUNTLET_WORKBENCH_ENDPOINT=http://127.0.0.1:4000 bun run src/main.ts; echo EXITED rc=\$?; sleep 3600"
LAUNCH_COLD="cd $ROOT/workbench && $ENVCOMMON GAUNTLET_WORKBENCH_ENDPOINT=http://127.0.0.1:9 bun run src/main.ts; echo EXITED rc=\$?; sleep 3600"
n=0
cap() { # $1 session, $2 state label
  n=$((n + 1))
  local nn size f
  nn=$(printf '%02d' "$n")
  size=$($T display-message -p -t "$1" '#{window_width}x#{window_height}')
  f="$OUT/${nn}-${size}-$2"
  $T capture-pane -p -t "$1" >"$f.txt"
  $T capture-pane -p -e -t "$1" >"$f.ansi"
  echo "$nn $size $2 md5=$(md5sum <"$f.txt" | cut -c1-8)"
}
key() { $T send-keys -t "$1" "${@:2}"; sleep 0.8; }
lit() { $T send-keys -t "$1" -l "$2"; sleep 0.8; }
rsz() { $T resize-window -t "$1" -x "$2" -y "$3"; sleep 1.5; }

# ---- populated leg (session base) ----
$T new-session -d -x 120 -y 40 -s base "$LAUNCH_POP"
$T set-option -t base window-size manual >/dev/null
sleep 5
cap base populated-initial
key base Tab;                cap base tab-from-primary
key base BTab; key base BTab; cap base shifttab-x2-navigator
key base Down; key base Down; cap base navigator-down2
key base Enter;              cap base navigator-enter-select
key base 2;                  cap base view-2-chat
key base 3;                  cap base view-3-trace
key base 1;                  cap base view-1-work
key base Escape;             cap base escape-to-primary
key base p;                  cap base palette-open
key base Down; key base Down; key base Down; cap base palette-down3
key base Escape;             cap base palette-escape
key base h;                  cap base hoist-h
key base u;                  cap base unhoist-u
key base e; lit base "hello draft"; cap base composer-draft
key base Escape;             cap base composer-escape
key base '?';                cap base questionmark-unbound
lit base $'\e[<0;30;10M'; lit base $'\e[<0;30;10m'; cap base sgr-click-30-10
lit base $'\e[<64;30;10M';   cap base sgr-wheel-30-10
rsz base 140 45;             cap base populated
rsz base 100 30;             cap base populated
rsz base 80 24;              cap base populated
key base Tab;                cap base compact-tab
key base Escape
key base p;                  cap base compact-palette
key base Escape
rsz base 72 20;              cap base below-minimum
rsz base 120 40;             cap base resize-recovered
key base q; sleep 1;         cap base after-q-quit

# ---- cold/unreachable leg (session cold) ----
$T new-session -d -x 120 -y 40 -s cold "$LAUNCH_COLD"
$T set-option -t cold window-size manual >/dev/null
sleep 5
cap cold cold-initial
key cold p;                  cap cold cold-palette-open
key cold Escape
rsz cold 140 45;             cap cold cold
rsz cold 100 30;             cap cold cold
rsz cold 80 24;              cap cold cold
rsz cold 72 20;              cap cold cold-below-minimum
key cold q; sleep 1;         cap cold cold-after-q-quit

$T kill-server
echo "teardown: tmux server gxrecon killed"
