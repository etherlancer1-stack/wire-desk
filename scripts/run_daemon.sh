#!/usr/bin/env bash
# Wire Desk watchdog daemon (spec #224).
# Runs the wired server inside tmux or a foreground loop with automatic restart
# and exponential crash backoff to prevent infinite tight crash loops.
set -euo pipefail

WIRED_BIN="${WIRED_BIN:./crates/wired/target/release/wired}"
DB="${WIREDESK_DB_PATH:-./data/wire-desk.db}"
HOST="${WIREDESK_HOST:-0.0.0.0}"
PORT="${WIREDESK_PORT:-8080}"
TMUX_SESSION="${TMUX_SESSION:-wired}"
USE_TMUX="${USE_TMUX:-1}"
MAX_BACKOFF=120
BACKOFF=5

log() { echo "[$(date -Is)] [watchdog] $*"; }

start_foreground() {
  log "starting wired (pid $$) on $HOST:$PORT db=$DB"
  exec env WIREDESK_DB_PATH="$DB" WIREDESK_HOST="$HOST" WIREDESK_PORT="$PORT" "$WIRED_BIN" serve --host "$HOST" --port "$PORT"
}

start_tmux() {
  if ! command -v tmux >/dev/null 2>&1; then
    log "tmux not found; falling back to foreground"
    start_foreground
    return
  fi
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    log "tmux session $TMUX_SESSION already exists; attaching"
    tmux attach-session -t "$TMUX_SESSION"
    return
  fi
  log "creating tmux session $TMUX_SESSION"
  tmux new-session -d -s "$TMUX_SESSION" "WIREDESK_DB_PATH='$DB' WIREDESK_HOST='$HOST' WIREDESK_PORT='$PORT' '$WIRED_BIN' serve --host '$HOST' --port '$PORT'"
  tmux attach-session -t "$TMUX_SESSION"
}

# Foreground watchdog loop with exponential backoff (spec #224).
watchdog() {
  log "watchdog started (max backoff ${MAX_BACKOFF}s)"
  while true; do
    log "launching wired"
    env WIREDESK_DB_PATH="$DB" WIREDESK_HOST="$HOST" WIREDESK_PORT="$PORT" "$WIRED_BIN" serve --host "$HOST" --port "$PORT"
    rc=$?
    log "wired exited with code $rc"
    if [ "$rc" -eq 0 ]; then
      log "clean exit; stopping watchdog"
      exit 0
    fi
    log "restarting in ${BACKOFF}s (exponential backoff)"
    sleep "$BACKOFF"
    BACKOFF=$((BACKOFF * 2))
    [ "$BACKOFF" -gt "$MAX_BACKOFF" ] && BACKOFF="$MAX_BACKOFF"
  done
}

case "${1:-watchdog}" in
  watchdog) watchdog ;;
  tmux) start_tmux ;;
  foreground|fg) start_foreground ;;
  *) echo "usage: $0 {watchdog|tmux|foreground}"; exit 2 ;;
esac
