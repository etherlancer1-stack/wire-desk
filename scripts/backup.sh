#!/usr/bin/env bash
# Wire Desk backup script (spec #225).
# SQLite safe backup/checkpoint + tar/gzip snapshot of DB, config, and reports
# into backups/ with timestamp and retention pruning.
set -euo pipefail

DB="${WIREDESK_DB_PATH:-./data/wire-desk.db}"
BACKUP_DIR="${WIREDESK_BACKUP_DIR:-./backups}"
RETENTION="${BACKUP_RETENTION_DAYS:-7}"
DATA_DIR="${WIREDESK_DATA_DIR:-./data}"
LOGS_DIR="${WIREDESK_LOG_DIR:-./logs}"

mkdir -p "$BACKUP_DIR"
TS=$(date +%Y%m%d-%H%M%S)
ARCHIVE="$BACKUP_DIR/wired-backup-$TS.tar.gz"

log() { echo "[$(date -Is)] [backup] $*"; }

log "starting backup -> $ARCHIVE"

# SQLite online backup to a temp file, then gzip.
TMP_DB="$(mktemp)"
sqlite3 "$DB" ".backup '$TMP_DB'" 2>/dev/null || cp "$DB" "$TMP_DB"
# Checkpoint WAL for a clean snapshot.
sqlite3 "$DB" "PRAGMA wal_checkpoint(TRUNCATE);" >/dev/null 2>&1 || true

tar -czf "$ARCHIVE" -C "$(dirname "$TMP_DB")" "$(basename "$TMP_DB")" \
  -C "$(dirname "$DATA_DIR")" "$(basename "$DATA_DIR")" \
  -C "$(dirname "$LOGS_DIR")" "$(basename "$LOGS_DIR")" 2>/dev/null || {
    # Fallback: just archive the DB and config.
    tar -czf "$ARCHIVE" "$TMP_DB" "$DATA_DIR/config.json" "$DATA_DIR/scope.allowlist.json" 2>/dev/null || true
  }
rm -f "$TMP_DB"

log "archive created: $ARCHIVE"

# Retention pruning (spec #84).
find "$BACKUP_DIR" -name "wired-backup-*.tar.gz" -mtime +"$RETENTION" -delete 2>/dev/null || true
log "retention pruned (>${RETENTION} days)"

echo "$ARCHIVE"
