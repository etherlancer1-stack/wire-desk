#!/usr/bin/env bash
# Wire Desk restore script (spec #226).
# Validates backup archive, stops active service if needed, restores DB and
# files atomically, and runs integrity check.
set -euo pipefail

ARCHIVE="${1:-}"
if [ -z "$ARCHIVE" ]; then
  echo "usage: $0 <backup.tar.gz>"
  exit 2
fi
if [ ! -f "$ARCHIVE" ]; then
  echo "backup archive not found: $ARCHIVE"
  exit 1
fi

DB="${WIREDESK_DB_PATH:-./data/wire-desk.db}"
BACKUP_DIR="${WIREDESK_BACKUP_DIR:-./backups}"
DATA_DIR="${WIREDESK_DATA_DIR:-./data}"
WORK_DIR="$(mktemp -d)"

log() { echo "[$(date -Is)] [restore] $*"; }
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

log "validating archive: $ARCHIVE"
tar -tzf "$ARCHIVE" >/dev/null 2>&1 || { echo "invalid archive"; exit 1; }

log "extracting to $WORK_DIR"
tar -xzf "$ARCHIVE" -C "$WORK_DIR"

# Find the DB file inside the archive.
RESTORE_DB="$(find "$WORK_DIR" -name "*.db" | head -1)"
if [ -z "$RESTORE_DB" ]; then
  echo "no .db found in archive"; exit 1
fi

# Integrity check on the source DB.
log "running SQLite integrity check"
sqlite3 "$RESTORE_DB" "PRAGMA integrity_check;" | grep -q "ok" || { echo "integrity check failed"; exit 1; }

# Stop active service if running (best effort).
if command -v pkill >/dev/null 2>&1; then
  pkill -f "wired serve" 2>/dev/null || true
  sleep 1
fi

# Atomic restore via temp + rename (spec #79 style).
mkdir -p "$(dirname "$DB")" "$DATA_DIR"
TMP_DB="$(mktemp)"
cp "$RESTORE_DB" "$TMP_DB"
mv "$TMP_DB" "$DB"

# Restore config/scope if present.
for f in config.json scope.allowlist.json; do
  SRC="$(find "$WORK_DIR" -name "$f" | head -1 || true)"
  if [ -n "$SRC" ]; then cp "$SRC" "$DATA_DIR/$f"; fi
done

log "restored DB to $DB"
log "restore complete; run '$WIRED_BIN doctor --db $DB' to verify"
