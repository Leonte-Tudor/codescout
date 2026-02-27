use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn open_db(project_root: &Path) -> Result<Connection> {
    let path = project_root.join(".code-explorer").join("usage.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS tool_calls (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name  TEXT NOT NULL,
            called_at  TEXT NOT NULL DEFAULT (datetime('now')),
            latency_ms INTEGER NOT NULL,
            outcome    TEXT NOT NULL,
            overflowed INTEGER NOT NULL DEFAULT 0,
            error_msg  TEXT
        );",
    )?;
    Ok(conn)
}

pub fn write_record(
    conn: &Connection,
    tool_name: &str,
    latency_ms: i64,
    outcome: &str,
    overflowed: bool,
    error_msg: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, error_msg)
         VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5)",
        params![tool_name, latency_ms, outcome, overflowed as i64, error_msg],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        (dir, conn)
    }

    #[test]
    fn open_db_creates_table() {
        let (_dir, conn) = tmp();
        // table exists if this doesn't error
        conn.execute("SELECT 1 FROM tool_calls LIMIT 0", [])
            .unwrap();
    }

    #[test]
    fn write_record_roundtrip() {
        let (_dir, conn) = tmp();
        write_record(&conn, "find_symbol", 42, "success", false, None).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn write_record_stores_all_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "semantic_search",
            150,
            "recoverable_error",
            false,
            Some("path not found"),
        )
        .unwrap();
        let (name, latency, outcome, overflowed, msg): (String, i64, String, i64, Option<String>) =
            conn.query_row(
                "SELECT tool_name, latency_ms, outcome, overflowed, error_msg FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(name, "semantic_search");
        assert_eq!(latency, 150);
        assert_eq!(outcome, "recoverable_error");
        assert_eq!(overflowed, 0);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn write_record_overflow_flag() {
        let (_dir, conn) = tmp();
        write_record(&conn, "list_symbols", 80, "success", true, None).unwrap();
        let overflowed: i64 = conn
            .query_row("SELECT overflowed FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(overflowed, 1);
    }

    #[test]
    fn retention_prunes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert a row with a timestamp 31 days ago
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('old_tool', datetime('now', '-31 days'), 10, 'success', 0)",
            [],
        )
        .unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 1);

        // Next write triggers pruning
        write_record(&conn, "new_tool", 5, "success", false, None).unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = 'old_tool'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, 0);
    }
}
