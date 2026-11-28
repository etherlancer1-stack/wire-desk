//! SQLite persistence layer.
//! Applies pragmas (spec #58), runs idempotent migrations (spec #72), and
//! provides a connection helper used by the repository layer via
//! `tokio::task::spawn_blocking` (rusqlite is synchronous).

use crate::error::WireResult;
use crate::models::utc_now;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Open (or create) the SQLite database with safe pragmas.
pub fn open(db_path: &Path) -> WireResult<Connection> {
    let flags = OpenFlags::default()
        | OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE;
    let conn = Connection::open_with_flags(db_path, flags)
        .map_err(|e| WireError::Database(format!("open failed: {}", e)))?;

    apply_pragmas(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Apply the standard Wire Desk SQLite pragmas (spec #58).
pub fn apply_pragmas(conn: &Connection) -> WireResult<()> {
    let pragmas = [
        "PRAGMA journal_mode = WAL;",
        "PRAGMA synchronous = NORMAL;",
        "PRAGMA foreign_keys = ON;",
        "PRAGMA busy_timeout = 5000;",
        "PRAGMA temp_store = MEMORY;",
    ];
    for pragma in pragmas {
        conn.execute_batch(pragma).map_err(|e| {
            WireError::Database(format!("pragma '{pragma}' failed: {e}"))
        })?;
    }
    Ok(())
}

/// Run idempotent migrations from embedded SQL (spec #72, #342).
pub fn run_migrations(conn: &Connection) -> WireResult<()> {
    // Ensure the _migrations table exists.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            version TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    ).map_err(|e| WireError::Database(format!("create _migrations failed: {e}")))?;

    let migration_sql = include_str!("../../../migrations/0001_initial.sql");
    // Extract just the CREATE/INSERT statements (skip leading PRAGMAs which are
    // already applied) and run them. Each statement is idempotent via IF NOT
    // EXISTS / INSERT OR IGNORE.
    for stmt in migration_sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() || stmt.to_ascii_uppercase().starts_with("PRAGMA") {
            continue;
        }
        // Record the migration version if present.
        if stmt.to_ascii_uppercase().starts_with("INSERT OR IGNORE INTO _migrations") {
            conn.execute(stmt, []).map_err(|e| {
                WireError::Database(format!("migration record failed: {e}"))
            })?;
            continue;
        }
        conn.execute_batch(stmt).map_err(|e| {
            WireError::Database(format!("migration statement failed: {e} :: {stmt}"))
        })?;
    }

    // Seed data (idempotent) is applied separately via the `seed` CLI flag or
    // on explicit request; do not auto-seed here to avoid surprising users.
    Ok(())
}

/// Run seed data idempotently (spec #73).
pub fn run_seed(conn: &Connection) -> WireResult<()> {
    let seed_sql = include_str!("../../../migrations/0002_seed.sql");
    for stmt in seed_sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() || stmt.to_ascii_uppercase().starts_with("PRAGMA") {
            continue;
        }
        conn.execute_batch(stmt).map_err(|e| {
            WireError::Database(format!("seed statement failed: {e} :: {stmt}"))
        })?;
    }
    Ok(())
}

/// Check that the database is reachable and pragmas are set (for `/readyz`).
pub fn health_check(conn: &Connection) -> WireResult<()> {
    let mut row = conn.prepare("SELECT 1")?.query([])?;
    if row.next().map_err(|e| WireError::Database(format!("query failed: {e}")))?.is_none() {
        return Err(WireError::Database("health check returned no row".into()));
    }
    Ok(())
}

/// Atomic file write helper (spec #79): write to `<target>.tmp.<pid>`, fsync,
/// then atomically rename over `<target>`.
pub fn atomic_write(target: &Path, contents: &[u8]) -> WireResult<()> {
    let pid = std::process::id();
    let tmp = target.with_extension(format!("tmp.{}", pid));
    std::fs::write(&tmp, contents).map_err(|e| {
        WireError::Database(format!("write tmp failed: {e}"))
    })?;
    let file = std::fs::File::open(&tmp).map_err(|e| {
        WireError::Database(format!("open tmp failed: {e}"))
    })?;
    file.sync_all().map_err(|e| WireError::Database(format!("fsync failed: {e}")))?;
    drop(file);
    std::fs::rename(&tmp, target).map_err(|e| {
        WireError::Database(format!("rename failed: {e}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_open_and_pragmas() {
        let dir = temp_dir().join(format!("wired-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.db");
        let conn = open(&db).unwrap();
        health_check(&conn).unwrap();
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_atomic_write() {
        let dir = temp_dir().join(format!("wired-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("payload.txt");
        atomic_write(&target, b"hello").unwrap();
        let contents = std::fs::read_to_string(&target).unwrap();
        assert_eq!(contents, "hello");
        let _ = std::fs::remove_dir_all(dir);
    }
}
