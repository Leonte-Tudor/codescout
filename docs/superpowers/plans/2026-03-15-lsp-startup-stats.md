# LSP Startup Statistics Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record LSP cold-start timing events (handshake + first real response) to `usage.db` and surface them in the project dashboard.

**Architecture:** New `lsp_events` table in the existing `.codescout/usage.db`. `do_start` in `LspManager` writes the handshake row immediately; a new `record_first_response` method on `LspProvider` completes the row when the first tool LSP call returns. Dashboard gets a new `/api/lsp` endpoint and a new UI section.

**Tech Stack:** Rust, rusqlite (existing), tokio, axum (existing dashboard), vanilla JS (existing dashboard).

**Spec:** `docs/superpowers/specs/2026-03-15-lsp-startup-stats-design.md`

---

## Chunk 1: DB layer — `lsp_events` table + write/read functions

### Task 1: Add `lsp_events` table and write functions to `src/usage/db.rs`

**Files:**
- Modify: `src/usage/db.rs`

- [ ] **Step 1: Write failing tests**

Add inside the existing `#[cfg(test)] mod tests` block in `src/usage/db.rs`:

```rust
#[test]
fn write_lsp_event_returns_rowid() {
    let (_dir, conn) = tmp();
    let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
    assert!(rowid > 0);
}

#[test]
fn update_lsp_first_response_fills_null() {
    let (_dir, conn) = tmp();
    let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
    // Before update: first_response_ms should be NULL
    let val: Option<i64> = conn
        .query_row(
            "SELECT first_response_ms FROM lsp_events WHERE id = ?",
            [rowid],
            |r| r.get(0),
        )
        .unwrap();
    assert!(val.is_none());
    // After update: should be set
    update_lsp_first_response(&conn, rowid, 9100).unwrap();
    let val: Option<i64> = conn
        .query_row(
            "SELECT first_response_ms FROM lsp_events WHERE id = ?",
            [rowid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(val, Some(9100));
}
```

- [ ] **Step 2: Run — expect compile error** (`write_lsp_event` not yet defined)

```bash
cargo test -p codescout write_lsp_event 2>&1 | head -20
```

- [ ] **Step 3: Add `lsp_events` table to `open_db` schema**

In `src/usage/db.rs`, inside `open_db`, append to the `execute_batch` SQL string:

```sql
CREATE TABLE IF NOT EXISTS lsp_events (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    language          TEXT NOT NULL,
    started_at        TEXT NOT NULL DEFAULT (datetime('now')),
    reason            TEXT NOT NULL,
    handshake_ms      INTEGER NOT NULL,
    first_response_ms INTEGER
);
```

- [ ] **Step 4: Add `write_lsp_event` and `update_lsp_first_response`**

Add after `write_record` in `src/usage/db.rs`:

```rust
/// Record an LSP cold-start event. Returns the inserted row id for the
/// two-phase write (first_response_ms is filled in later by `update_lsp_first_response`).
pub fn write_lsp_event(
    conn: &Connection,
    language: &str,
    reason: &str,
    handshake_ms: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO lsp_events (language, reason, handshake_ms) VALUES (?1, ?2, ?3)",
        params![language, reason, handshake_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Fill in the first_response_ms for a previously inserted lsp_events row.
/// Best-effort — if the row was already updated or is missing, this is a no-op.
pub fn update_lsp_first_response(
    conn: &Connection,
    rowid: i64,
    first_response_ms: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE lsp_events SET first_response_ms = ?1 WHERE id = ?2 AND first_response_ms IS NULL",
        params![first_response_ms, rowid],
    )?;
    Ok(())
}
```

- [ ] **Step 5: Run tests — expect pass**

```bash
cargo test -p codescout write_lsp_event update_lsp_first_response 2>&1
```

Expected: both tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/usage/db.rs
git commit -m "feat(usage): add lsp_events table with write_lsp_event and update_lsp_first_response"
```

---

### Task 2: Add `query_lsp_stats` to `src/usage/db.rs`

**Files:**
- Modify: `src/usage/db.rs`

- [ ] **Step 1: Write failing tests**

Add inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn query_lsp_stats_aggregates_correctly() {
    let (_dir, conn) = tmp();
    write_lsp_event(&conn, "rust", "new_session", 800).unwrap();
    write_lsp_event(&conn, "rust", "idle_evicted", 1200).unwrap();
    write_lsp_event(&conn, "kotlin", "new_session", 5000).unwrap();

    let stats = query_lsp_stats(&conn, "30d").unwrap();
    assert_eq!(stats.by_language.len(), 2);

    let rust = stats.by_language.iter().find(|l| l.language == "rust").unwrap();
    assert_eq!(rust.starts, 2);
    assert_eq!(rust.reasons.new_session, 1);
    assert_eq!(rust.reasons.idle_evicted, 1);
    assert_eq!(rust.avg_handshake_ms, 1000); // (800 + 1200) / 2
    assert!(rust.p95_handshake_ms >= 800);

    let kotlin = stats.by_language.iter().find(|l| l.language == "kotlin").unwrap();
    assert_eq!(kotlin.starts, 1);
    assert_eq!(kotlin.avg_handshake_ms, 5000);
}

#[test]
fn query_lsp_stats_window_excludes_old_rows() {
    let (_dir, conn) = tmp();
    // Insert an old row manually with an ancient timestamp
    conn.execute(
        "INSERT INTO lsp_events (language, started_at, reason, handshake_ms)
         VALUES ('rust', datetime('now', '-60 days'), 'new_session', 999)",
        [],
    )
    .unwrap();
    // Insert a recent row
    write_lsp_event(&conn, "rust", "new_session", 800).unwrap();

    let stats = query_lsp_stats(&conn, "30d").unwrap();
    let rust = stats.by_language.iter().find(|l| l.language == "rust").unwrap();
    // Only the recent row should be counted
    assert_eq!(rust.starts, 1);
    assert_eq!(rust.avg_handshake_ms, 800);
}

#[test]
fn query_lsp_stats_recent_returns_last_20() {
    let (_dir, conn) = tmp();
    for i in 0..25i64 {
        write_lsp_event(&conn, "rust", "new_session", i * 10).unwrap();
    }
    let stats = query_lsp_stats(&conn, "30d").unwrap();
    assert_eq!(stats.recent.len(), 20);
}
```

- [ ] **Step 2: Run — expect compile error**

```bash
cargo test -p codescout query_lsp_stats 2>&1 | head -20
```

- [ ] **Step 3: Add structs and `query_lsp_stats`**

Add after `UsageStats` in `src/usage/db.rs`:

```rust
#[derive(Debug, Default, serde::Serialize)]
pub struct LspReasonCounts {
    pub new_session: i64,
    pub idle_evicted: i64,
    pub lru_evicted: i64,
    pub crashed: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct LspLanguageStats {
    pub language: String,
    pub starts: i64,
    pub reasons: LspReasonCounts,
    pub avg_handshake_ms: i64,
    pub p95_handshake_ms: i64,
    pub avg_first_response_ms: Option<i64>,
    pub p95_first_response_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspEvent {
    pub language: String,
    pub started_at: String,
    pub reason: String,
    pub handshake_ms: i64,
    pub first_response_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspStats {
    pub window: String,
    pub by_language: Vec<LspLanguageStats>,
    pub recent: Vec<LspEvent>,
}

pub fn query_lsp_stats(conn: &Connection, window: &str) -> Result<LspStats> {
    let modifier = window_to_modifier(window);

    // Aggregate per language
    let mut agg_stmt = conn.prepare(
        "SELECT language,
                COUNT(*) as starts,
                SUM(CASE WHEN reason = 'new_session'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN reason = 'idle_evicted' THEN 1 ELSE 0 END),
                SUM(CASE WHEN reason = 'lru_evicted'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN reason = 'crashed'      THEN 1 ELSE 0 END),
                AVG(handshake_ms),
                AVG(first_response_ms)
         FROM lsp_events
         WHERE started_at >= datetime('now', ?)
         GROUP BY language
         ORDER BY starts DESC",
    )?;

    let rows: Vec<(String, i64, i64, i64, i64, i64, f64, Option<f64>)> = agg_stmt
        .query_map([modifier], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut by_language = Vec::new();
    for (language, starts, new_session, idle_evicted, lru_evicted, crashed, avg_handshake, avg_first) in rows {
        let p95_handshake = lsp_percentile(conn, &language, modifier, 95, "handshake_ms")?;
        // `.ok()` is intentional: `p95_first_response_ms` is an Optional field in the response.
        // `lsp_percentile` returns `Ok(0)` when count=0 (all NULL values), so the only case
        // `.ok()` silently discards is a genuine DB error — acceptable for a best-effort
        // observability field.
        let p95_first = lsp_percentile(conn, &language, modifier, 95, "first_response_ms").ok();

        by_language.push(LspLanguageStats {
            language,
            starts,
            reasons: LspReasonCounts { new_session, idle_evicted, lru_evicted, crashed },
            avg_handshake_ms: avg_handshake.round() as i64,
            p95_handshake_ms: p95_handshake,
            avg_first_response_ms: avg_first.map(|v| v.round() as i64),
            p95_first_response_ms: p95_first,
        });
    }

    // Recent events (last 20, not window-filtered — always shows the most recent cold starts
    // regardless of the selected window, so the list is never empty while data exists)
    let mut recent_stmt = conn.prepare(
        "SELECT language, started_at, reason, handshake_ms, first_response_ms
         FROM lsp_events
         ORDER BY started_at DESC
         LIMIT 20",
    )?;
    let recent: Vec<LspEvent> = recent_stmt
        .query_map([], |r| {
            Ok(LspEvent {
                language: r.get(0)?,
                started_at: r.get(1)?,
                reason: r.get(2)?,
                handshake_ms: r.get(3)?,
                first_response_ms: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(LspStats { window: window.to_string(), by_language, recent })
}

/// Nearest-rank p95 for a numeric column in `lsp_events`, scoped to one language.
/// Returns `Ok(0)` when there are no rows, `Err` only on DB failure.
fn lsp_percentile(
    conn: &Connection,
    language: &str,
    modifier: &str,
    pct: i64,
    column: &str,
) -> Result<i64> {
    // Only count non-NULL values for the given column
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT({}) FROM lsp_events
             WHERE language = ? AND started_at >= datetime('now', ?) AND {} IS NOT NULL",
            column, column
        ),
        params![language, modifier],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    let offset = ((count * pct + 99) / 100 - 1).max(0);
    let val: i64 = conn.query_row(
        &format!(
            "SELECT {} FROM lsp_events
             WHERE language = ? AND started_at >= datetime('now', ?) AND {} IS NOT NULL
             ORDER BY {} LIMIT 1 OFFSET ?",
            column, column, column
        ),
        params![language, modifier, offset],
        |r| r.get(0),
    )?;
    Ok(val)
}
```

> **Note:** `lsp_percentile` uses `format!` to interpolate the column name — this is safe because `column` is only ever called with the string literals `"handshake_ms"` and `"first_response_ms"`, never user input. Do not expose `column` as a public parameter.

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p codescout query_lsp_stats 2>&1
```

Expected: all 3 new tests pass, no regressions.

- [ ] **Step 5: Lint + fmt**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 6: Commit**

```bash
git add src/usage/db.rs
git commit -m "feat(usage): add query_lsp_stats with per-language aggregates and recent events"
```

---

## Chunk 2: LspManager — write path + reason tracking

### Task 3: Add `pending_first_response`, `pending_reason`, and instrument `do_start`

**Files:**
- Modify: `src/lsp/manager.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)] mod tests` in `src/lsp/manager.rs`:

```rust
#[tokio::test]
async fn do_start_records_lsp_event_to_db() {
    // Use a real temp dir so open_db works
    let dir = tempfile::TempDir::new().unwrap();
    let mgr = LspManager::new_for_test_with_root(dir.path()).await;

    // `get_or_start_for_test` takes (language, config) — workspace root comes from config
    mgr.get_or_start_for_test("rust", fake_lsp_config(dir.path()))
        .await
        .unwrap();

    // Verify an lsp_events row was written
    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM lsp_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);

    let (lang, reason): (String, String) = conn
        .query_row(
            "SELECT language, reason FROM lsp_events LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(lang, "rust");
    assert_eq!(reason, "new_session");
}

#[tokio::test]
async fn do_start_reason_evicted_consumes_pending_reason() {
    let dir = tempfile::TempDir::new().unwrap();
    let mgr = LspManager::new_for_test_with_root(dir.path()).await;
    let key = LspKey::new("rust", dir.path());

    // Pre-populate pending_reason as if eviction happened
    mgr.pending_reason
        .lock()
        .unwrap()
        .insert(key, "idle_evicted".to_string());

    mgr.get_or_start_for_test("rust", fake_lsp_config(dir.path()))
        .await
        .unwrap();

    // pending_reason should be consumed
    assert!(mgr.pending_reason.lock().unwrap().is_empty());

    // DB row should have reason = idle_evicted
    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let reason: String = conn
        .query_row("SELECT reason FROM lsp_events LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(reason, "idle_evicted");
}
```

> **Note:** `LspManager::new_for_test_with_root` does not exist yet — it will be added in Step 3. This test will fail to compile until then.
> `get_or_start_for_test` signature is `(language: &str, config: LspServerConfig)` — the workspace root is derived from `config.workspace_root`, not passed separately.

- [ ] **Step 2: Run — expect compile error**

```bash
cargo test -p codescout do_start_records_lsp_event 2>&1 | head -20
```

- [ ] **Step 3: Add new fields and helpers to `LspManager`**

In `src/lsp/manager.rs`, extend the `LspManager` struct with two new fields:

```rust
pub struct LspManager {
    // ... existing fields unchanged ...
    /// Maps LspKey → (db rowid, cold-start Instant) for the two-phase write.
    /// Populated by do_start; consumed (first-caller-wins) by record_first_response.
    pending_first_response: StdMutex<HashMap<LspKey, (i64, std::time::Instant)>>,
    /// Reason for the next cold start of a given key, set by eviction paths before
    /// removing the client. Consumed by do_start (defaults to "new_session" if absent).
    pub(crate) pending_reason: StdMutex<HashMap<LspKey, String>>,
}
```

Update `LspManager::new()` (inside `new_arc_with_ttl` or wherever `Self { ... }` is constructed) to initialise both new fields:

```rust
pending_first_response: StdMutex::new(HashMap::new()),
pending_reason: StdMutex::new(HashMap::new()),
```

Add a test-only constructor that carries a project root so `do_start` can open `usage.db`:

```rust
#[cfg(test)]
pub async fn new_for_test_with_root(project_root: &std::path::Path) -> Arc<Self> {
    let mut mgr = Self::new();
    mgr.project_root_for_test = Some(project_root.to_path_buf());
    Arc::new(mgr)
}
```

> **`project_root_for_test` is a `#[cfg(test)]`-only struct field.** In Rust, conditional struct fields require every `Self { ... }` expression to also guard the field with `#[cfg(test)]`. Concretely: in `LspManager::new()` (and any other place that constructs `LspManager` with a literal `Self { ... }`), wrap the field initialiser:
> ```rust
> Self {
>     // ... existing fields ...
>     #[cfg(test)]
>     project_root_for_test: None,
> }
> ```
> Omitting this `#[cfg(test)]` guard on the initialiser will produce a compile error in non-test builds ("no field `project_root_for_test` on type `LspManager`").

- [ ] **Step 4: Instrument `do_start`**

Inside `do_start`, after the `let result = LspClient::start(config).await.map(Arc::new);` line and inside the `Ok(new_client)` arm, add:

```rust
// Record LSP startup event — best-effort, never fail the startup.
let reason = self
    .pending_reason
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .remove(key)
    .unwrap_or_else(|| "new_session".to_string());
let handshake_ms = start_time.elapsed().as_millis() as i64;
tracing::info!(
    "LSP initialized in {}ms (language: {}, reason: {})",
    handshake_ms, key.language, reason
);
#[cfg(test)]
let project_root_opt = self.project_root_for_test.clone();
#[cfg(not(test))]
let project_root_opt: Option<std::path::PathBuf> = None;
if let Some(root) = project_root_opt {
    if let Ok(conn) = crate::usage::db::open_db(&root) {
        if let Ok(rowid) = crate::usage::db::write_lsp_event(
            &conn, &key.language, &reason, handshake_ms,
        ) {
            self.pending_first_response
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key.clone(), (rowid, std::time::Instant::now()));
        }
    }
}
```

> **Note:** `start_time` must be placed **immediately before `LspClient::start(config).await`**, not at the top of `do_start`. `do_start` first shuts down any stale dead client (`old.shutdown().await`), which can take up to 35 s. Including that in `handshake_ms` would inflate the measurement. Place `start_time` after the stale-client eviction block, right before the `LspClient::start` call. Also add `pending_reason` entries in the `evict_idle` and LRU eviction paths (see Task 4).

- [ ] **Step 5: Run tests — expect pass**

```bash
cargo test -p codescout do_start_records_lsp_event do_start_reason_evicted 2>&1
```

- [ ] **Step 6: Lint**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 7: Commit**

```bash
git add src/lsp/manager.rs
git commit -m "feat(lsp): instrument do_start — record lsp_events on cold start with reason tracking"
```

---

### Task 4: Set `pending_reason` in eviction paths

**Files:**
- Modify: `src/lsp/manager.rs`

- [ ] **Step 1: Add `pending_reason` entries in `evict_idle`**

Inside `evict_idle`, just before `clients.remove(&key)`:

```rust
self.pending_reason
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .insert(key.clone(), "idle_evicted".to_string());
```

- [ ] **Step 2: Add `pending_reason` entry in the LRU eviction block in `get_or_start`**

Inside the `if clients.len() >= self.max_clients` block, just before the `clients.remove(&oldest_key)` call:

```rust
self.pending_reason
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .insert(oldest_key.clone(), "lru_evicted".to_string());
```

- [ ] **Step 3: Run all manager tests**

```bash
cargo test -p codescout -- src/lsp/manager 2>&1 | tail -30
```

Expected: all existing tests pass, no new failures.

- [ ] **Step 4: Commit**

```bash
git add src/lsp/manager.rs
git commit -m "feat(lsp): set pending_reason for idle and LRU eviction paths"
```

---

## Chunk 3: `record_first_response` — read path + tool sites

### Task 5: Add `record_first_response` to `LspProvider` trait and `LspManager`

**Files:**
- Modify: `src/lsp/ops.rs`
- Modify: `src/lsp/manager.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests` in `src/lsp/manager.rs`:

```rust
#[tokio::test]
async fn record_first_response_consumes_pending_and_updates_db() {
    let dir = tempfile::TempDir::new().unwrap();
    let mgr = LspManager::new_for_test_with_root(dir.path()).await;

    // Start the LSP to create the pending entry
    mgr.get_or_start_for_test("rust", fake_lsp_config(dir.path()))
        .await
        .unwrap();

    // First call should consume the pending entry and write to DB
    mgr.record_first_response("rust", dir.path(), 9100).await;

    // pending_first_response should now be empty
    assert!(mgr.pending_first_response.lock().unwrap().is_empty());

    // DB row should be updated
    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let val: Option<i64> = conn
        .query_row(
            "SELECT first_response_ms FROM lsp_events LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(val, Some(9100));
}

#[tokio::test]
async fn record_first_response_noop_when_no_pending() {
    let dir = tempfile::TempDir::new().unwrap();
    let mgr = LspManager::new_for_test_with_root(dir.path()).await;
    // No prior get_or_start — calling record_first_response should not panic or error
    mgr.record_first_response("rust", dir.path(), 5000).await;
}

#[tokio::test]
async fn record_first_response_second_call_is_noop() {
    let dir = tempfile::TempDir::new().unwrap();
    let mgr = LspManager::new_for_test_with_root(dir.path()).await;

    mgr.get_or_start_for_test("rust", fake_lsp_config(dir.path()))
        .await
        .unwrap();

    mgr.record_first_response("rust", dir.path(), 9100).await;
    // Second call — pending is already consumed, should be a silent no-op
    mgr.record_first_response("rust", dir.path(), 1234).await;

    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let val: Option<i64> = conn
        .query_row(
            "SELECT first_response_ms FROM lsp_events LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // Should still be 9100 — second call didn't overwrite
    assert_eq!(val, Some(9100));
}
```

- [ ] **Step 2: Run — expect compile error**

```bash
cargo test -p codescout record_first_response 2>&1 | head -20
```

- [ ] **Step 3: Add default method to `LspProvider` trait**

In `src/lsp/ops.rs`, inside the `LspProvider` trait block:

```rust
/// Record the first real LSP response time for a cold-started client.
/// Default implementation is a no-op — only `LspManager` does real work.
/// Best-effort: implementations must never propagate errors.
async fn record_first_response(
    &self,
    _language: &str,
    _workspace_root: &std::path::Path,
    _elapsed_ms: i64,
) {
}
```

- [ ] **Step 4: Implement `record_first_response_inner` on `LspManager` and wire trait**

> **Important — name collision:** if both the inherent impl and the trait impl use the same method name `record_first_response`, calling `self.record_first_response(...)` inside the trait impl resolves to the trait method itself (infinite recursion). To avoid this, name the inherent method `record_first_response_inner` and have the trait impl call it.

Add to `impl LspManager` in `src/lsp/manager.rs`:

```rust
/// Inner implementation of first-response recording. Called by the LspProvider
/// trait impl. Named `_inner` to avoid the infinite-recursion trap where
/// `self.record_first_response(...)` inside a trait impl resolves back to the
/// trait method rather than this inherent method.
pub async fn record_first_response_inner(
    &self,
    language: &str,
    workspace_root: &std::path::Path,
    elapsed_ms: i64,
) {
    let key = LspKey::new(language, workspace_root);
    let pending = self
        .pending_first_response
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&key);

    let Some((rowid, _)) = pending else { return };

    tracing::debug!("LSP first response in {}ms (language: {})", elapsed_ms, language);

    #[cfg(test)]
    let project_root_opt = self.project_root_for_test.clone();
    #[cfg(not(test))]
    let project_root_opt: Option<std::path::PathBuf> = None;

    let Some(root) = project_root_opt else { return };

    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = crate::usage::db::open_db(&root) {
            let _ = crate::usage::db::update_lsp_first_response(&conn, rowid, elapsed_ms);
        }
    })
    .await;
}
```

In the `impl LspProvider for LspManager` block, override the trait method to call `_inner`:

```rust
async fn record_first_response(
    &self,
    language: &str,
    workspace_root: &std::path::Path,
    elapsed_ms: i64,
) {
    // Call the inherent method by name to avoid infinite recursion
    // (self.record_first_response(...) would resolve back to this trait method)
    LspManager::record_first_response_inner(self, language, workspace_root, elapsed_ms).await;
}
```

- [ ] **Step 5: Run tests — expect pass**

```bash
cargo test -p codescout record_first_response 2>&1
```

- [ ] **Step 6: Lint**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 7: Commit**

```bash
git add src/lsp/ops.rs src/lsp/manager.rs
git commit -m "feat(lsp): add record_first_response to LspProvider trait and LspManager"
```

---

### Task 6: Instrument the 6 LSP tool call sites

**Files:**
- Modify: `src/tools/symbol.rs`

All 5 tools live in `src/tools/symbol.rs`. The call sites to instrument:

1. `find_symbol` single-file path — via `get_lsp_client` helper → `document_symbols` call (~line 494)
2. `list_symbols` single-file path — via `get_lsp_client` → `document_symbols` (~line 491)
3. `find_symbol` directory scan — first `get_or_start` + `document_symbols` in loop (~line 458)
4. `list_symbols` directory scan — first `get_or_start` + `document_symbols` in loop (~line 638)
5. `goto_definition` — its primary `get_or_start` + LSP call site
6. `hover` — its primary `get_or_start` + LSP call site
7. `find_references` — via `get_lsp_client` → already covered if Step 1/2 instrument at the `get_lsp_client` call site (since `pending_first_response` is consumed first-caller-wins)

> **Language string:** `record_first_response` must receive the **same language string** used to build the `LspKey` in `do_start` (e.g. `"rust"`, `"kotlin"`). Do **not** call `detect_language` again at the call site — instead reuse the `lang` variable already in scope from `ctx.lsp.get_or_start(lang, &root)`. This guarantees key alignment.

> **Note:** Only wrap the **first** LSP call after `get_or_start` at each site. Do not instrument internal helpers (`resolve_range_via_document_symbols`, `workspace_symbols`). Since `pending_first_response` is consumed on first call, later calls are silent no-ops.

- [ ] **Step 1: Instrument `find_symbol` + `find_references` single-file paths** (both use `get_lsp_client`)

`get_lsp_client` returns `(Arc<dyn LspClientOps>, language_id_string)`. It does not itself make an LSP call, so instrument at the call sites. The raw language string (e.g. `"rust"`) needed for `record_first_response` must be derived from `detect_language` **before** calling `get_lsp_client`, since `get_lsp_client` only exposes the LSP language-id string (same value in practice, but reuse the `lang` variable for correctness):

```rust
// Pattern to apply at each get_lsp_client call site:
let lang = crate::ast::detect_language(&full_path)
    .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
let root = ctx.agent.require_project_root().await?;
let (client, language_id) = get_lsp_client(ctx, &full_path).await?;
let lsp_timer = std::time::Instant::now();
let symbols = client.document_symbols(&full_path, &language_id).await?;
let _ = ctx.lsp.record_first_response(lang, &root, lsp_timer.elapsed().as_millis() as i64).await;
```

> `lang` is the raw language key (`"rust"`, `"kotlin"`) — the same string passed to `get_or_start` inside `get_lsp_client`. Using the variable already in scope avoids a redundant `detect_language` call and guarantees key alignment with `do_start`.

- [ ] **Step 2: Instrument `list_symbols` single-file path** (~line 491)

Same pattern as Step 1 — `lang` is already in scope from the `detect_language` call that gates the block.

- [ ] **Step 3: Instrument `find_symbol` directory scan path** (~line 458 — first iteration only)

Wrap the first `document_symbols` call in the loop. `lang` is already in scope:

```rust
if let Ok(client) = ctx.lsp.get_or_start(lang, &root).await {
    let lsp_timer = std::time::Instant::now();
    if let Ok(symbols) = client.document_symbols(file_path, language_id).await {
        let _ = ctx.lsp.record_first_response(lang, &root, lsp_timer.elapsed().as_millis() as i64).await;
        // ... rest of loop body unchanged
    }
}
```

- [ ] **Step 4: Instrument `list_symbols` directory scan path** (~line 638)

Same pattern as Step 3.

- [ ] **Step 5: Instrument `goto_definition`**

Find the `goto_definition` tool's `call` method in `src/tools/symbol.rs`. It calls `get_or_start` then makes an LSP request. Apply the same pattern:

```rust
let client = ctx.lsp.get_or_start(lang, &root).await?;
let lsp_timer = std::time::Instant::now();
let result = client.goto_definition(&path, line, col, language_id).await?;
let _ = ctx.lsp.record_first_response(lang, &root, lsp_timer.elapsed().as_millis() as i64).await;
```

- [ ] **Step 6: Instrument `hover`**

Same pattern in the `hover` tool's `call` method.

- [ ] **Step 7: Verify `find_references` is covered**

`find_references` calls `get_lsp_client` then `document_symbols` — the same path instrumented in Steps 1 and 2. Since `pending_first_response` is consumed first-caller-wins, no additional instrumentation is needed; confirm it shares the `get_lsp_client` call path and move on.

- [ ] **Step 8: Build to check for compile errors**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

- [ ] **Step 9: Run full test suite**

```bash
cargo test 2>&1 | tail -30
```

Expected: all existing tests pass.

- [ ] **Step 10: Lint**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 11: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(tools): record first LSP response time at 6 call sites (find_symbol, list_symbols, find_references, goto_definition, hover)"
```

---

## Chunk 4: Dashboard API + UI

### Task 7: Add `/api/lsp` endpoint

> **Depends on:** Task 2. Before starting, verify: `grep -n "query_lsp_stats" src/usage/db.rs` must return a result. If it doesn't, complete Tasks 1-6 first.

**Files:**
- Create: `src/dashboard/api/lsp.rs`
- Modify: `src/dashboard/api/mod.rs`
- Modify: `src/dashboard/routes.rs`

- [ ] **Step 1: Create `src/dashboard/api/lsp.rs`**

Model exactly after `src/dashboard/api/usage.rs`:

```rust
use axum::{extract::{Query, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::routes::DashboardState;

#[derive(Deserialize)]
pub struct LspParams {
    pub window: Option<String>,
}

pub async fn get_lsp(
    State(state): State<DashboardState>,
    Query(params): Query<LspParams>,
) -> Json<Value> {
    let db_path = state.project_root.join(".codescout").join("usage.db");
    if !db_path.exists() {
        return Json(json!({
            "available": false,
            "reason": "No usage data recorded yet."
        }));
    }

    let conn = match crate::usage::db::open_db(&state.project_root) {
        Ok(c) => c,
        Err(e) => {
            return Json(json!({
                "available": false,
                "reason": format!("Failed to open usage DB: {}", e)
            }));
        }
    };

    let window = params.window.as_deref().unwrap_or("30d");
    match crate::usage::db::query_lsp_stats(&conn, window) {
        Ok(stats) => {
            let mut val = serde_json::to_value(stats).unwrap_or_default();
            val["available"] = json!(true);
            Json(val)
        }
        Err(e) => Json(json!({
            "available": false,
            "reason": format!("Query failed: {}", e)
        })),
    }
}
```

- [ ] **Step 2: Register in `src/dashboard/api/mod.rs`**

Add:
```rust
pub mod lsp;
```

- [ ] **Step 3: Register route in `src/dashboard/routes.rs`**

Add to the `Router::new()` chain:
```rust
.route("/api/lsp", get(api::lsp::get_lsp))
```

Also add to the `mod tests` block in `routes.rs`:

```rust
#[tokio::test]
async fn lsp_returns_not_available_without_db() {
    let dir = tempfile::TempDir::new().unwrap();
    let app = test_router(dir.path());
    let req = Request::builder()
        .uri("/api/lsp")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["available"], false);
}
```

- [ ] **Step 4: Build with dashboard feature**

```bash
cargo build --features dashboard 2>&1 | grep "^error" | head -20
```

- [ ] **Step 5: Run dashboard tests**

```bash
cargo test --features dashboard -- dashboard 2>&1 | tail -20
```

- [ ] **Step 6: Lint**

```bash
cargo fmt && cargo clippy --features dashboard -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 7: Commit**

```bash
git add src/dashboard/api/lsp.rs src/dashboard/api/mod.rs src/dashboard/routes.rs
git commit -m "feat(dashboard): add /api/lsp endpoint for LSP startup statistics"
```

---

### Task 8: Add LSP Startup section to dashboard UI

**Files:**
- Modify: `src/dashboard/static/index.html`
- Modify: `src/dashboard/static/dashboard.js`

- [ ] **Step 1: Add HTML skeleton to `index.html`**

Find the existing tool stats section (look for `id="usage-summary"` or `id="usage-table"`). Add this block immediately after it:

```html
<section id="lsp-section">
  <h2>LSP Startup</h2>
  <div id="lsp-table"></div>
  <h3>Recent Events</h3>
  <div id="lsp-recent"></div>
</section>
```

- [ ] **Step 2: Add LSP fetch and render to `dashboard.js`**

In `refreshStats()`, add `fetchJson('/api/lsp?window=' + win)` to the `Promise.all` array **and** update the destructuring assignment to capture the new result:

```javascript
// Before (existing):
const [usage, errors] = await Promise.all([
    fetchJson('/api/usage?window=' + win),
    fetchJson('/api/errors?limit=20'),
]);

// After:
const [usage, errors, lsp] = await Promise.all([
    fetchJson('/api/usage?window=' + win),
    fetchJson('/api/errors?limit=20'),
    fetchJson('/api/lsp?window=' + win),
]);
```

Then add rendering logic after the existing usage render block:

```javascript
// LSP startup section
if (lsp && lsp.available) {
    const langs = lsp.by_language || [];
    if (langs.length > 0) {
        const thead = '<tr><th>Language</th><th>Starts</th><th>Reasons</th>' +
            '<th class="num">Avg handshake</th><th class="num">p95 handshake</th>' +
            '<th class="num">Avg first resp</th><th class="num">p95 first resp</th></tr>';
        const rows = langs.map(l => {
            const r = l.reasons || {};
            const badges = [
                r.new_session    ? r.new_session    + ' new'     : '',
                r.idle_evicted   ? r.idle_evicted   + ' evicted' : '',
                r.lru_evicted    ? r.lru_evicted    + ' lru'     : '',
                r.crashed        ? r.crashed        + ' crash'   : '',
            ].filter(Boolean).join(' · ');
            const fmtMs = ms => ms == null ? '—' :
                ms >= 1000 ? (ms / 1000).toFixed(1) + 's' : ms + 'ms';
            return '<tr><td>' + esc(l.language) + '</td>' +
                '<td class="num">' + l.starts + '</td>' +
                '<td>' + esc(badges) + '</td>' +
                '<td class="num">' + fmtMs(l.avg_handshake_ms) + '</td>' +
                '<td class="num">' + fmtMs(l.p95_handshake_ms) + '</td>' +
                '<td class="num">' + fmtMs(l.avg_first_response_ms) + '</td>' +
                '<td class="num">' + fmtMs(l.p95_first_response_ms) + '</td></tr>';
        }).join('');
        document.getElementById('lsp-table').innerHTML =
            '<table>' + thead + '<tbody>' + rows + '</tbody></table>';
    } else {
        document.getElementById('lsp-table').innerHTML =
            '<p class="muted">No LSP startup events in this window.</p>';
    }

    // Recent events
    const recent = lsp.recent || [];
    if (recent.length > 0) {
        const items = recent.map(e => {
            const fmtMs = ms => ms == null ? '' : ' · first resp ' +
                (ms >= 1000 ? (ms / 1000).toFixed(1) + 's' : ms + 'ms');
            return '<li>[' + esc(e.language) + '] ' + esc(e.reason) +
                ' · handshake ' + e.handshake_ms + 'ms' +
                fmtMs(e.first_response_ms) +
                ' · <span class="muted">' + esc(e.started_at) + '</span></li>';
        }).join('');
        document.getElementById('lsp-recent').innerHTML = '<ul>' + items + '</ul>';
    } else {
        document.getElementById('lsp-recent').innerHTML =
            '<p class="muted">No recent LSP events.</p>';
    }
} else {
    document.getElementById('lsp-table').innerHTML =
        '<p class="muted">' + esc((lsp && lsp.reason) || 'No LSP data available.') + '</p>';
    document.getElementById('lsp-recent').innerHTML = '';
}
```

- [ ] **Step 3: Verify with dev server**

```bash
cargo run --features dashboard -- dashboard --project . --port 8080
```

Open `http://localhost:8080` — the "LSP Startup" section should be visible. If no events in the DB yet, it shows the "No LSP startup events" placeholder.

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/static/index.html src/dashboard/static/dashboard.js
git commit -m "feat(dashboard): add LSP Startup section with aggregate table and recent events"
```

---

## Chunk 5: Wire production DB path + final integration

### Task 9: Thread project root to `LspManager` for production writes

> **Depends on:** Tasks 3 and 5 (prior approved chunks). Before starting, verify both are applied:
> ```bash
> grep -n "project_root_for_test\|record_first_response_inner" src/lsp/manager.rs
> ```
> Both must return results. If not, complete Chunks 2-3 first.

**Files:**
- Modify: `src/lsp/manager.rs`
- Modify: `src/server.rs` (where `LspManager` is constructed — lines 53 and 354)

Currently the production `do_start` skips DB writes because only the test path has a project root. This task wires the real project root.

- [ ] **Step 1: Add `project_root` field to `LspManager`**

Add the `project_root` field to the struct definition (the `project_root_for_test` field was already added by Task 3):

```rust
pub struct LspManager {
    // ... existing fields (including #[cfg(test)] project_root_for_test from Task 3) ...
    /// Project root for usage.db writes. Set at construction time.
    project_root: Option<std::path::PathBuf>,
}
```

Also add `project_root: None` to `LspManager::new()`:

```rust
// In LspManager::new() struct literal:
project_root: None,
```

- [ ] **Step 2: Extract `new_arc_inner` and add `new_arc_with_root`**

`new_arc_with_ttl` is called directly in tests with one argument — **do not change its signature**. Instead, extract the shared implementation into a private helper, and have both existing constructors and the new one delegate to it:

```rust
/// Shared implementation — carries the project root; not pub.
fn new_arc_inner(ttl: Duration, project_root: Option<std::path::PathBuf>) -> Arc<Self> {
    let mut mgr = Self::new();
    mgr.idle_ttl = ttl;
    mgr.project_root = project_root;
    let arc = Arc::new(mgr);
    let weak = Arc::downgrade(&arc);
    tokio::spawn(async move {
        Self::idle_eviction_loop(weak, ttl).await;
    });
    arc
}

// Update both existing pub constructors to delegate to new_arc_inner:
pub fn new_arc() -> Arc<Self> {
    Self::new_arc_inner(Duration::from_secs(30 * 60), None)
}
pub fn new_arc_with_ttl(ttl: Duration) -> Arc<Self> {  // signature unchanged
    Self::new_arc_inner(ttl, None)
}

/// Production constructor: like `new_arc` but writes startup timing to usage.db.
pub fn new_arc_with_root(project_root: std::path::PathBuf) -> Arc<Self> {
    Self::new_arc_inner(Duration::from_secs(30 * 60), Some(project_root))
}
```

- [ ] **Step 3: Update `do_start` and `record_first_response_inner` to use `self.project_root`**

Replace the `#[cfg(test)] / #[cfg(not(test))]` blocks with:

```rust
let project_root_opt = self.project_root.clone();
#[cfg(test)]
let project_root_opt = self.project_root_for_test.clone().or(project_root_opt);
```

- [ ] **Step 5: Wire production call sites in `src/server.rs`**

```bash
grep -r "LspManager::new_arc" src/server.rs
```

Two sites — both currently call `LspManager::new_arc()`. Replace them as follows.

**Line 53** — `CodeScoutServer::new(agent: Agent)`. The agent exposes `project_root().await -> Option<PathBuf>`:

```rust
pub async fn new(agent: Agent) -> Self {
    let lsp = match agent.project_root().await {
        Some(root) => LspManager::new_arc_with_root(root),
        None => LspManager::new_arc(),
    };
    Self::from_parts(agent, lsp).await
}
```

**Line 354** — `run(...)`. At this point `project` is `Option<PathBuf>`:

```rust
let lsp = match project.clone() {
    Some(root) => LspManager::new_arc_with_root(root),
    None => LspManager::new_arc(),
};
```

All other `LspManager::new_arc()` call sites in `src/tools/` remain unchanged.

- [ ] **Step 6: Run all tests**

```bash
cargo test 2>&1 | tail -30
```

- [ ] **Step 7: Build release binary to verify MCP integration**

```bash
cargo build --release 2>&1 | grep "^error" | head -20
```

- [ ] **Step 8: Final lint**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo clippy --features dashboard -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 9: Commit**

```bash
git add src/lsp/manager.rs src/server.rs
git commit -m "feat(lsp): wire project root to LspManager for production lsp_events DB writes"
```

---

## Chunk 6: Experimental docs + final checks

### Task 10: Write experimental docs and run full verification

**Files:**
- Create: `docs/manual/src/experimental/lsp-startup-stats.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create experimental doc**

```markdown
# LSP Startup Statistics

> ⚠ Experimental — may change without notice.

codescout records LSP cold-start timing to `.codescout/usage.db` and surfaces it
in the project dashboard under "LSP Startup".

## What is recorded

Each cold start records:
- **Language** — which LSP server was started
- **Reason** — `new_session`, `idle_evicted`, `lru_evicted`, or `crashed`
- **Handshake duration** — time for the LSP `initialize` round trip
- **First response duration** — time for the first real tool request (symbols, hover, etc.)

## Viewing the data

Open the dashboard (`codescout dashboard --project .`) and look for the
"LSP Startup" section. It shows per-language averages/p95 and a recent event list.

## Limitations

- `first_response_ms` may be `null` if no tool call followed the cold start in the
  same server process.
- Events are only recorded when the project root is known at startup time.
```

- [ ] **Step 2: Add entry to `docs/manual/src/experimental/index.md`**

```markdown
- [LSP Startup Statistics](./lsp-startup-stats.md)
```

- [ ] **Step 3: Run full test suite one final time**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -40
```

Expected: all tests pass, no clippy warnings.

- [ ] **Step 4: Commit everything**

```bash
git add docs/manual/src/experimental/lsp-startup-stats.md docs/manual/src/experimental/index.md
git commit -m "docs(experimental): add LSP startup statistics page"
```

---

## Summary

| Chunk | Tasks | Key files |
|---|---|---|
| 1 — DB layer | 1–2 | `src/usage/db.rs` |
| 2 — LspManager write path | 3–4 | `src/lsp/manager.rs` |
| 3 — Read path + tool sites | 5–6 | `src/lsp/ops.rs`, `src/lsp/manager.rs`, `src/tools/symbol.rs` |
| 4 — Dashboard | 7–8 | `src/dashboard/api/lsp.rs`, `routes.rs`, `index.html`, `dashboard.js` |
| 5 — Production wiring | 9 | `src/lsp/manager.rs`, `src/agent.rs` |
| 6 — Docs | 10 | `docs/manual/src/experimental/` |
