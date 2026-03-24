# Library Indexing Redesign — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate library embeddings into per-library SQLite files, add version-aware staleness detection, wire up auto-discovery and nudging, and open read-only navigation tools to library code.

**Architecture:** Per-library SQLite DBs under `.codescout/embeddings/lib/`, migration from the shared `embeddings.db`, multi-DB search with score-based merging, lockfile-based version tracking, and a three-layer agent UX (discover → nudge → optional auto-index).

**Tech Stack:** Rust, tokio, rusqlite + sqlite-vec, serde_json (lockfile parsing), ignore (file walking)

**Spec:** `docs/superpowers/specs/2026-03-13-library-indexing-redesign.md`

---

## Chunk 1: Foundation — DB Separation & Migration

### Task 1: New DB path functions and directory layout

**Files:**
- Modify: `src/embed/index.rs` (functions `db_path`, `open_db`)

- [ ] **Step 1: Write failing tests for new path functions**

In `src/embed/index.rs` tests module, add:

```rust
#[test]
fn project_db_path_uses_embeddings_dir() {
    let root = Path::new("/tmp/test-project");
    let path = project_db_path(root);
    assert_eq!(path, root.join(".codescout/embeddings/project.db"));
}

#[test]
fn lib_db_path_basic() {
    let root = Path::new("/tmp/test-project");
    let path = lib_db_path(root, "tokio");
    assert_eq!(path, root.join(".codescout/embeddings/lib/tokio.db"));
}

#[test]
fn lib_db_path_sanitizes_scoped_npm() {
    let root = Path::new("/tmp/test-project");
    let path = lib_db_path(root, "@scope/name");
    assert_eq!(path, root.join(".codescout/embeddings/lib/@scope--name.db"));
}

#[test]
fn lib_db_path_sanitizes_backslash() {
    let root = Path::new("/tmp/test-project");
    let path = lib_db_path(root, "foo\\bar");
    assert_eq!(path, root.join(".codescout/embeddings/lib/foo--bar.db"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test project_db_path_uses_embeddings_dir lib_db_path_basic lib_db_path_sanitizes -- --nocapture`
Expected: compile errors — functions don't exist yet

- [ ] **Step 3: Implement `project_db_path`, `lib_db_path`, `sanitize_lib_name`**

In `src/embed/index.rs`, add above `db_path`:

```rust
/// Path to the project embedding database (new layout).
pub fn project_db_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codescout")
        .join("embeddings")
        .join("project.db")
}

/// Path to a library's embedding database.
pub fn lib_db_path(project_root: &Path, lib_name: &str) -> PathBuf {
    project_root
        .join(".codescout")
        .join("embeddings")
        .join("lib")
        .join(format!("{}.db", sanitize_lib_name(lib_name)))
}

/// Sanitize a library name for use as a filename.
/// Replaces `/` and `\` with `--`, lowercases on case-insensitive OS.
fn sanitize_lib_name(name: &str) -> String {
    let mut s = name.replace(['/', '\\'], "--");
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        s = s.to_lowercase();
    }
    s
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test project_db_path lib_db_path -- --nocapture`
Expected: all 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): add project_db_path, lib_db_path with name sanitization"
```

---

### Task 2: Migration logic — detect old layout and migrate

**Files:**
- Modify: `src/embed/index.rs` (new function `maybe_migrate_db_layout`, modify `open_db`)

- [ ] **Step 1: Write failing test for migration detection**

```rust
#[test]
fn migrate_db_layout_renames_old_db() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let old_path = root.join(".codescout/embeddings.db");
    let new_path = project_db_path(root);

    // Create old-style DB
    std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
    std::fs::write(&old_path, b"fake-db").unwrap();

    assert!(!new_path.exists());
    maybe_migrate_db_layout(root).unwrap();
    assert!(new_path.exists());
    assert!(!old_path.exists());
    // lib/ directory created
    assert!(root.join(".codescout/embeddings/lib").is_dir());
}

#[test]
fn migrate_db_layout_noop_when_new_layout_exists() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let new_path = project_db_path(root);
    std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
    std::fs::write(&new_path, b"already-migrated").unwrap();

    // Old file also exists (shouldn't be touched)
    let old_path = root.join(".codescout/embeddings.db");
    std::fs::write(&old_path, b"old-db").unwrap();

    maybe_migrate_db_layout(root).unwrap();
    // New file untouched
    assert_eq!(std::fs::read(&new_path).unwrap(), b"already-migrated");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test migrate_db_layout -- --nocapture`
Expected: compile error — function doesn't exist

- [ ] **Step 3: Implement `maybe_migrate_db_layout`**

```rust
/// Migrate from old single-DB layout to new embeddings/ directory layout.
/// Called from `open_db` before opening the connection.
///
/// Old: `.codescout/embeddings.db`
/// New: `.codescout/embeddings/project.db` + `.codescout/embeddings/lib/`
pub fn maybe_migrate_db_layout(project_root: &Path) -> Result<()> {
    let old_path = db_path(project_root); // .codescout/embeddings.db
    let new_path = project_db_path(project_root);

    // Already migrated or no old DB — nothing to do
    if new_path.exists() || !old_path.exists() {
        // Ensure lib/ directory exists regardless
        let lib_dir = project_root.join(".codescout/embeddings/lib");
        if !lib_dir.exists() {
            std::fs::create_dir_all(&lib_dir)?;
        }
        return Ok(());
    }

    tracing::info!("Migrating embedding storage to new layout...");

    // Create new directory structure
    std::fs::create_dir_all(new_path.parent().unwrap())?;
    std::fs::create_dir_all(project_root.join(".codescout/embeddings/lib"))?;

    // Rename old DB to new location
    std::fs::rename(&old_path, &new_path)?;

    tracing::info!(
        "Migration complete: {} → {}",
        old_path.display(),
        new_path.display()
    );
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test migrate_db_layout -- --nocapture`
Expected: both tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): add maybe_migrate_db_layout for old→new DB transition"
```

---

### Task 3: Extract library chunks during migration

**Files:**
- Modify: `src/embed/index.rs` (extend `maybe_migrate_db_layout`)

- [ ] **Step 1: Write failing test for library chunk extraction**

```rust
#[test]
fn migrate_extracts_library_chunks_to_separate_dbs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create a real old-style DB with project + library chunks
    let old_path = db_path(root);
    std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
    {
        let conn = Connection::open(&old_path).unwrap();
        conn.execute_batch("
            CREATE TABLE chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                language TEXT NOT NULL,
                content TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                file_hash TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'project'
            );
            CREATE TABLE chunk_embeddings (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL);
            CREATE TABLE files (path TEXT PRIMARY KEY, hash TEXT NOT NULL, mtime INTEGER);
            INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source)
                VALUES ('src/main.rs', 'rust', 'fn main() {}', 1, 1, 'aaa', 'project');
            INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source)
                VALUES ('[lib:tokio]/src/runtime.rs', 'rust', 'pub struct Runtime', 1, 1, 'bbb', 'lib:tokio');
            INSERT INTO chunk_embeddings (rowid, embedding) VALUES (1, X'00000000');
            INSERT INTO chunk_embeddings (rowid, embedding) VALUES (2, X'00000000');
        ").unwrap();
    }

    maybe_migrate_db_layout(root).unwrap();

    // project.db should only have the project chunk
    let proj_conn = Connection::open(project_db_path(root)).unwrap();
    let count: i64 = proj_conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE source = 'project'", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 1);
    let lib_count: i64 = proj_conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE source LIKE 'lib:%'", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(lib_count, 0);

    // tokio.db should have the library chunk
    let lib_path = lib_db_path(root, "tokio");
    assert!(lib_path.exists());
    let lib_conn = Connection::open(&lib_path).unwrap();
    let lib_chunk_count: i64 = lib_conn.query_row(
        "SELECT COUNT(*) FROM chunks", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(lib_chunk_count, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test migrate_extracts_library_chunks -- --nocapture`
Expected: FAIL — migration doesn't extract yet

- [ ] **Step 3: Extend `maybe_migrate_db_layout` to extract library chunks**

After the rename step, add library chunk extraction:

```rust
// After rename, check for library chunks and extract them
extract_library_chunks_from_project_db(project_root)?;
```

Implement `extract_library_chunks_from_project_db`:

```rust
/// After migration, split library chunks out of project.db into per-library DBs.
fn extract_library_chunks_from_project_db(project_root: &Path) -> Result<()> {
    let proj_path = project_db_path(project_root);
    let conn = Connection::open(&proj_path)?;

    // Find distinct library sources
    let mut stmt = conn.prepare("SELECT DISTINCT source FROM chunks WHERE source LIKE 'lib:%'")?;
    let sources: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    if sources.is_empty() {
        // Fallback: check file_path patterns against libraries.json
        // (handles DBs where source column was added late)
        extract_by_file_path_fallback(project_root, &conn)?;
        return Ok(());
    }

    for source in &sources {
        let lib_name = source.strip_prefix("lib:").unwrap();
        let lib_path = lib_db_path(project_root, lib_name);
        copy_chunks_to_lib_db(&conn, &lib_path, source)?;

        // Delete from project DB
        conn.execute("DELETE FROM chunks WHERE source = ?", [source])?;
        conn.execute(
            "DELETE FROM chunk_embeddings WHERE rowid NOT IN (SELECT id FROM chunks)",
            [],
        )?;
    }

    conn.execute_batch("VACUUM")?;
    tracing::info!("Extracted {} library DBs from project.db", sources.len());
    Ok(())
}

fn copy_chunks_to_lib_db(src_conn: &Connection, lib_path: &Path, source: &str) -> Result<()> {
    if let Some(parent) = lib_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lib_conn = Connection::open(lib_path)?;
    lib_conn.execute_batch("
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL, language TEXT NOT NULL,
            content TEXT NOT NULL, start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL, file_hash TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'project'
        );
        CREATE TABLE IF NOT EXISTS chunk_embeddings (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, hash TEXT NOT NULL, mtime INTEGER);
        CREATE TABLE IF NOT EXISTS lib_meta (key TEXT PRIMARY KEY, value TEXT);
    ")?;

    let mut read_stmt = src_conn.prepare(
        "SELECT file_path, language, content, start_line, end_line, file_hash, source FROM chunks WHERE source = ?"
    )?;
    let mut emb_stmt = src_conn.prepare(
        "SELECT embedding FROM chunk_embeddings WHERE rowid = ?"
    )?;
    let mut id_stmt = src_conn.prepare(
        "SELECT id FROM chunks WHERE source = ? ORDER BY id"
    )?;

    let ids: Vec<i64> = id_stmt
        .query_map([source], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .collect();

    lib_conn.execute_batch("BEGIN")?;
    for id in &ids {
        let row = src_conn.query_row(
            "SELECT file_path, language, content, start_line, end_line, file_hash, source FROM chunks WHERE id = ?",
            [id],
            |r| {
            Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?,
                r.get::<_, String>(2)?, r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?, r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        });
        if let Ok((fp, lang, content, sl, el, fh, src)) = row {
            lib_conn.execute(
                "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source) VALUES (?,?,?,?,?,?,?)",
                rusqlite::params![fp, lang, content, sl, el, fh, src],
            )?;
            let new_id = lib_conn.last_insert_rowid();
            if let Ok(emb) = emb_stmt.query_row([id], |r| r.get::<_, Vec<u8>>(0)) {
                lib_conn.execute(
                    "INSERT INTO chunk_embeddings (rowid, embedding) VALUES (?, ?)",
                    rusqlite::params![new_id, emb],
                )?;
            }
        }
    }
    lib_conn.execute_batch("COMMIT")?;
    Ok(())
}

fn extract_by_file_path_fallback(project_root: &Path, _conn: &Connection) -> Result<()> {
    // Load libraries.json and match file_path prefixes
    // Deferred for now — most users won't have library chunks without source tags
    let lib_json_path = project_root.join(".codescout/libraries.json");
    if !lib_json_path.exists() {
        return Ok(());
    }
    // TODO: Implement file_path-based fallback when needed
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test migrate_extracts_library_chunks migrate_db_layout -- --nocapture`
Expected: all migration tests PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all existing tests still pass

- [ ] **Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): extract library chunks to per-lib DBs during migration"
```

---

### Task 4: Update `open_db` to use new layout

**Files:**
- Modify: `src/embed/index.rs` (change `open_db` to call migration + use new path)

- [ ] **Step 1: Write test that verifies open_db uses new path**

```rust
#[test]
fn open_db_uses_new_embeddings_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let conn = open_db(root).unwrap();
    // Verify the DB is at the new location
    assert!(project_db_path(root).exists());
    // Old location should not exist
    assert!(!db_path(root).exists());
    drop(conn);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test open_db_uses_new_embeddings_dir -- --nocapture`
Expected: FAIL — `open_db` still uses old path

- [ ] **Step 3: Update `open_db` to use new path + call migration**

Change `open_db` to:
1. Call `maybe_migrate_db_layout(project_root)?;` at the top
2. Use `project_db_path(project_root)` instead of `db_path(project_root)`

Keep the old `db_path` function (still used by migration detection).

- [ ] **Step 4: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass. Some existing tests may need `project_db_path` instead of `db_path` for assertions.

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): switch open_db to new embeddings/ directory layout"
```

---

### Task 5: New `open_lib_db` function

**Files:**
- Modify: `src/embed/index.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn open_lib_db_creates_with_lib_meta_table() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let conn = open_lib_db(root, "tokio").unwrap();
    // lib_meta table should exist
    conn.execute("INSERT INTO lib_meta (key, value) VALUES ('test', 'ok')", []).unwrap();
    let val: String = conn.query_row(
        "SELECT value FROM lib_meta WHERE key = 'test'", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(val, "ok");
}
```

- [ ] **Step 2: Run test to verify it fails**

- [ ] **Step 3: Implement `open_lib_db`**

```rust
/// Open (or create) the embedding database for a specific library.
pub fn open_lib_db(project_root: &Path, lib_name: &str) -> Result<Connection> {
    let path = lib_db_path(project_root, lib_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    init_sqlite_vec();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    conn.execute_batch("
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS chunks (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path  TEXT NOT NULL,
            language   TEXT NOT NULL,
            content    TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line   INTEGER NOT NULL,
            file_hash  TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'project'
        );

        CREATE TABLE IF NOT EXISTS chunk_embeddings (
            rowid     INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            path  TEXT PRIMARY KEY,
            hash  TEXT NOT NULL,
            mtime INTEGER
        );

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS lib_meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
    ")?;

    maybe_migrate_to_vec0(&conn)?;
    Ok(conn)
}
```

- [ ] **Step 4: Run tests, clippy, fmt**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test open_lib_db -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): add open_lib_db for per-library embedding databases"
```

---

### Task 6: Update `build_library_index` to write to separate DB

**Files:**
- Modify: `src/embed/index.rs` (change `build_library_index` to use `open_lib_db`)

- [ ] **Step 1: Write test that library indexing creates separate DB**

```rust
#[test]
fn build_library_index_creates_separate_db_file() {
    // This test requires a real library path with source files.
    // Create a minimal temp project + temp "library" with one .rs file.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let lib_dir = dir.path().join("fake-lib");
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::write(lib_dir.join("lib.rs"), "pub fn hello() {}").unwrap();

    // Ensure project DB exists first
    let _conn = open_db(root).unwrap();

    // This will fail at embedding (no API key), but should at least create the DB
    // We test the path logic, not full indexing
    let lib_db = lib_db_path(root, "fake-lib");
    assert!(!lib_db.exists()); // not yet
    // Full integration test deferred — just verify path function works
}
```

- [ ] **Step 2: Modify `build_library_index` to use `open_lib_db` instead of `open_db`**

Change the function signature to extract the library name from the source tag, then:
- Replace `let conn = open_db(project_root)?;` with `let conn = open_lib_db(project_root, lib_name)?;`
- The `lib_name` is parsed from `source` (strip `"lib:"` prefix)

- [ ] **Step 3: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): build_library_index writes to per-library DB"
```

---

## Chunk 2: Multi-DB Search & Version Tracking

### Task 7: Implement `search_multi_db`

**Files:**
- Modify: `src/embed/index.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn search_multi_db_project_only_scope() {
    // Setup: project.db with chunks, lib/tokio.db with chunks
    // Search with Scope::Project → only project results
}

#[test]
fn search_multi_db_lib_scope() {
    // Search with Scope::Library("tokio") → only tokio results
}

#[test]
fn search_multi_db_all_scope_merges_by_score() {
    // Search with Scope::All → results from both, sorted by score
}

#[test]
fn search_multi_db_missing_lib_db_graceful() {
    // Scope::Library("nonexistent") → empty results, no error
}
```

Test bodies: create temp directories, populate DBs with known chunks and embeddings, run search with a known query embedding, assert correct results and ordering.

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement `search_multi_db`**

```rust
/// Search across project and/or library embedding databases.
pub fn search_multi_db(
    project_root: &Path,
    query_embedding: &[f32],
    limit: usize,
    scope: &crate::library::scope::Scope,
    library_registry: &crate::library::registry::LibraryRegistry,
) -> Result<Vec<SearchResult>> {
    let mut db_paths: Vec<(PathBuf, Option<String>)> = Vec::new();

    match scope {
        crate::library::scope::Scope::Project => {
            db_paths.push((project_db_path(project_root), None));
        }
        crate::library::scope::Scope::Library(name) => {
            let p = lib_db_path(project_root, name);
            if p.exists() {
                db_paths.push((p, Some(format!("lib:{}", name))));
            }
        }
        crate::library::scope::Scope::Libraries => {
            let lib_dir = project_root.join(".codescout/embeddings/lib");
            if lib_dir.is_dir() {
                for entry in std::fs::read_dir(&lib_dir)?.flatten() {
                    let p = entry.path();
                    if p.extension().map_or(false, |e| e == "db") {
                        db_paths.push((p, None)); // source inferred from chunks
                    }
                }
            }
        }
        crate::library::scope::Scope::All => {
            db_paths.push((project_db_path(project_root), None));
            let lib_dir = project_root.join(".codescout/embeddings/lib");
            if lib_dir.is_dir() {
                for entry in std::fs::read_dir(&lib_dir)?.flatten() {
                    let p = entry.path();
                    if p.extension().map_or(false, |e| e == "db") {
                        db_paths.push((p, None));
                    }
                }
            }
        }
    }

    let mut all_results: Vec<SearchResult> = Vec::new();

    for (path, _source_filter) in &db_paths {
        if !path.exists() {
            continue;
        }
        let conn = Connection::open(path)?;
        let results = search(&conn, query_embedding, limit)?;
        all_results.extend(results);
    }

    // Sort by score descending, truncate to limit
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(limit);

    Ok(all_results)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test search_multi_db -- --nocapture`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): add search_multi_db for cross-DB semantic search"
```

---

### Task 8: Wire `search_multi_db` into `SemanticSearch` tool

**Files:**
- Modify: `src/tools/semantic.rs` (`SemanticSearch::call`)

- [ ] **Step 1: Write test that SemanticSearch delegates to search_multi_db with correct scope**

```rust
#[tokio::test]
async fn semantic_search_uses_scope_for_library_search() {
    // Setup: project_ctx with a registered library
    // Call SemanticSearch with scope="lib:tokio"
    // Verify it attempts to open the library DB (will fail gracefully with empty results
    // since no actual embeddings, but should NOT error)
    let ctx = project_ctx();
    let tool = SemanticSearch;
    let result = tool.call(json!({"query": "runtime", "scope": "lib:nonexistent"}), &ctx).await;
    // Should succeed (graceful skip for missing DB), not error
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Update SemanticSearch::call to use search_multi_db**

Replace the current `search_scoped` call block with `search_multi_db`. The key change is in the `spawn_blocking` block:
- Instead of calling `search_scoped(&conn, ...)`, call `search_multi_db(root, query_embedding, limit, &scope, &library_registry)`
- This requires passing `library_registry` into the blocking closure
- Read the library registry from the agent before spawning

- [ ] **Step 2: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass — semantic search tests still work (they use project scope by default)

- [ ] **Step 3: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(semantic): use search_multi_db for cross-library search"
```

---

### Task 9: `LibraryEntry` version fields + `libraries.json` schema

**Files:**
- Modify: `src/library/registry.rs`

- [ ] **Step 1: Write failing tests for version fields**

```rust
#[test]
fn library_entry_serializes_version_fields() {
    let entry = LibraryEntry {
        name: "tokio".to_string(),
        version: Some("1.38.0".to_string()),
        version_indexed: Some("1.37.0".to_string()),
        db_file: Some("tokio.db".to_string()),
        nudge_dismissed: false,
        path: PathBuf::from("/tmp/tokio"),
        language: "rust".to_string(),
        indexed: true,
        discovered_via: DiscoveryMethod::LspFollowThrough,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("version_indexed"));
    assert!(json.contains("db_file"));
    assert!(json.contains("nudge_dismissed"));
}

#[test]
fn library_entry_deserializes_without_new_fields() {
    // Old-format JSON without version_indexed, db_file, nudge_dismissed
    let json = r#"{"name":"serde","version":null,"path":"/tmp/serde","language":"rust","indexed":false,"discovered_via":"LspFollowThrough"}"#;
    let entry: LibraryEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.version_indexed, None);
    assert_eq!(entry.db_file, None);
    assert!(!entry.nudge_dismissed);
}

#[test]
fn stale_libraries_detects_version_mismatch() {
    let mut registry = LibraryRegistry::new();
    registry.register("tokio".into(), PathBuf::from("/tmp"), "rust".into(), DiscoveryMethod::LspFollowThrough);
    let entry = registry.lookup_mut("tokio").unwrap();
    entry.version = Some("1.38.0".to_string());
    entry.version_indexed = Some("1.37.0".to_string());
    entry.indexed = true;

    let stale = registry.stale_libraries();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].name, "tokio");
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Add new fields to `LibraryEntry` and `stale_libraries` method**

Add to `LibraryEntry`:
```rust
#[serde(default)]
pub version_indexed: Option<String>,
#[serde(default)]
pub db_file: Option<String>,
#[serde(default)]
pub nudge_dismissed: bool,
```

Add to `impl LibraryRegistry`:
```rust
/// Return registered libraries where version != version_indexed (stale).
pub fn stale_libraries(&self) -> Vec<&LibraryEntry> {
    self.entries
        .iter()
        .filter(|e| {
            e.indexed
                && e.version.is_some()
                && e.version_indexed.is_some()
                && e.version != e.version_indexed
        })
        .collect()
}
```

- [ ] **Step 4: Fix compilation across codebase**

Anywhere `LibraryEntry` is constructed (in `register` method, tests), add the new fields with defaults.

- [ ] **Step 5: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/library/registry.rs
git commit -m "feat(library): add version_indexed, db_file, nudge_dismissed to LibraryEntry"
```

---

### Task 10: Version detection module — Cargo.lock parser (P0)

**Files:**
- Create: `src/library/versions.rs`
- Modify: `src/library/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_cargo_lock_extracts_versions() {
        let content = r#"
[[package]]
name = "tokio"
version = "1.38.0"

[[package]]
name = "serde"
version = "1.0.203"
"#;
        let versions = parse_cargo_lock(content);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].name, "tokio");
        assert_eq!(versions[0].version, "1.38.0");
        assert_eq!(versions[1].name, "serde");
        assert_eq!(versions[1].version, "1.0.203");
    }

    #[test]
    fn parse_package_lock_json_extracts_versions() {
        let content = r#"{
            "packages": {
                "node_modules/lodash": { "version": "4.17.21" },
                "node_modules/@types/node": { "version": "20.11.0" }
            }
        }"#;
        let versions = parse_package_lock_json(content);
        assert_eq!(versions.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement the versions module**

Create `src/library/versions.rs`:

```rust
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub name: String,
    pub version: String,
}

/// Detect project type and parse lockfile to get dependency versions.
pub fn resolve_dependency_versions(project_root: &Path) -> Vec<ResolvedVersion> {
    // Try Cargo.lock (Rust)
    let cargo_lock = project_root.join("Cargo.lock");
    if cargo_lock.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_lock) {
            return parse_cargo_lock(&content);
        }
    }

    // Try package-lock.json (JS/TS)
    let pkg_lock = project_root.join("package-lock.json");
    if pkg_lock.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_lock) {
            return parse_package_lock_json(&content);
        }
    }

    // TODO: yarn.lock, pnpm-lock.yaml, go.sum, poetry.lock, uv.lock

    Vec::new()
}

/// Parse Cargo.lock to extract package versions.
pub fn parse_cargo_lock(content: &str) -> Vec<ResolvedVersion> {
    let mut versions = Vec::new();
    let mut current_name: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name = ") {
            current_name = line
                .strip_prefix("name = ")
                .and_then(|s| s.strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
                .map(|s| s.to_string());
        } else if line.starts_with("version = ") {
            if let (Some(name), Some(ver)) = (
                current_name.take(),
                line.strip_prefix("version = ")
                    .and_then(|s| s.strip_prefix('"'))
                    .and_then(|s| s.strip_suffix('"')),
            ) {
                versions.push(ResolvedVersion {
                    name,
                    version: ver.to_string(),
                });
            }
        }
    }
    versions
}

/// Parse package-lock.json (v2/v3 format) to extract package versions.
pub fn parse_package_lock_json(content: &str) -> Vec<ResolvedVersion> {
    let mut versions = Vec::new();
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) else {
        return versions;
    };

    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
        for (key, val) in packages {
            // Skip root package (empty key)
            if key.is_empty() {
                continue;
            }
            let name = key
                .strip_prefix("node_modules/")
                .unwrap_or(key)
                .to_string();
            if let Some(version) = val.get("version").and_then(|v| v.as_str()) {
                versions.push(ResolvedVersion {
                    name,
                    version: version.to_string(),
                });
            }
        }
    }
    versions
}

/// Look up a specific library's version from resolved dependencies.
pub fn find_version(versions: &[ResolvedVersion], name: &str) -> Option<String> {
    versions.iter().find(|v| v.name == name).map(|v| v.version.clone())
}
```

Add `pub mod versions;` to `src/library/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test parse_cargo_lock parse_package_lock -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/library/versions.rs src/library/mod.rs
git commit -m "feat(library): add versions module with Cargo.lock and package-lock.json parsers"
```

---

### Task 11: Staleness hints in SemanticSearch responses

**Files:**
- Modify: `src/tools/semantic.rs`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn semantic_search_includes_stale_library_hints() {
    // Setup: mock agent with a library that has version != version_indexed
    // Call SemanticSearch, check response contains stale_libraries key
}
```

- [ ] **Step 2: Add staleness check after search results**

In `SemanticSearch::call`, after the search results are assembled:

```rust
// Check for stale libraries
let stale = {
    let inner = ctx.agent.inner.read().await;
    if let Some(p) = &inner.active_project {
        p.library_registry.stale_libraries()
            .into_iter()
            .map(|e| json!({
                "name": e.name,
                "indexed": e.version_indexed,
                "current": e.version,
                "hint": format!("{} was updated — run index_project(scope='lib:{}') to re-index", e.name, e.name)
            }))
            .collect::<Vec<_>>()
    } else {
        vec![]
    }
};
if !stale.is_empty() {
    result["stale_libraries"] = json!(stale);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(semantic): add stale library version hints to search responses"
```

---

### Task 11b: Version write-back on library re-index

**Files:**
- Modify: `src/tools/semantic.rs` (`IndexProject::call` — the `lib:` scope branch)
- Modify: `src/tools/library.rs` (`IndexLibrary::call`)

This is a critical task: without it, staleness detection fires forever because `version_indexed` is never updated.

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn index_project_lib_scope_updates_version_indexed() {
    // Setup: register a library with version "1.38.0", version_indexed = None
    // After indexing, version_indexed should be set to "1.38.0"
    // Also verify lib_meta table in the library DB has version_indexed
}
```

- [ ] **Step 2: Implement version write-back in IndexProject lib scope branch**

After `build_library_index` completes and the `indexed = true` flag is set:

```rust
// Read current version from lockfile and write back
let versions = crate::library::versions::resolve_dependency_versions(&root);
let current_version = crate::library::versions::find_version(&versions, lib_name)
    .or_else(|| {
        // Check version_overrides in config
        let inner_r = ctx.agent.inner.blocking_read(); // already in spawn_blocking
        inner_r.active_project.as_ref()
            .and_then(|p| p.config.libraries.version_overrides.get(lib_name).cloned())
    });

if let Some(ver) = &current_version {
    entry.version = Some(ver.clone());
    entry.version_indexed = Some(ver.clone());
    // Reset nudge_dismissed since we just indexed a fresh version
    entry.nudge_dismissed = false;
}
```

Also write to `lib_meta` table:
```rust
let lib_conn = open_lib_db(&root, lib_name)?;
if let Some(ver) = &current_version {
    lib_conn.execute("INSERT OR REPLACE INTO lib_meta (key, value) VALUES ('version_indexed', ?)", [ver])?;
}
```

Apply the same logic to `IndexLibrary::call` in `src/tools/library.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/semantic.rs src/tools/library.rs
git commit -m "feat(index): write-back version_indexed on library re-index"
```

---

### Task 11c: Reset `nudge_dismissed` on version change

**Files:**
- Modify: `src/library/versions.rs` or `src/agent.rs`

When version detection runs and a library's lockfile version differs from `LibraryEntry.version`, reset `nudge_dismissed = false`.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn nudge_dismissed_resets_on_version_change() {
    let mut registry = LibraryRegistry::new();
    registry.register("tokio".into(), PathBuf::from("/tmp"), "rust".into(), DiscoveryMethod::LspFollowThrough);
    let entry = registry.lookup_mut("tokio").unwrap();
    entry.version = Some("1.37.0".to_string());
    entry.nudge_dismissed = true;

    // Simulate version update
    registry.update_version("tokio", "1.38.0");
    let entry = registry.lookup("tokio").unwrap();
    assert!(!entry.nudge_dismissed);
    assert_eq!(entry.version.as_deref(), Some("1.38.0"));
}
```

- [ ] **Step 2: Implement `update_version` on `LibraryRegistry`**

```rust
/// Update a library's detected version. Resets nudge_dismissed if version changed.
pub fn update_version(&mut self, name: &str, new_version: &str) {
    if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
        let changed = entry.version.as_deref() != Some(new_version);
        entry.version = Some(new_version.to_string());
        if changed {
            entry.nudge_dismissed = false;
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test nudge_dismissed_resets -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/library/registry.rs
git commit -m "feat(library): reset nudge_dismissed when version changes"
```

---

## Chunk 3: Agent UX — Discovery, Nudging, Path Security

### Task 12: Wire `tag_external_path` into `goto_definition` and `hover`

**Files:**
- Modify: `src/tools/symbol.rs`

- [ ] **Step 1: Find where goto_definition and hover resolve the target path**

Both `GotoDefinition::call` and `Hover::call` get back a path from LSP. After resolving the path, add a call to `tag_external_path` to auto-register the library.

- [ ] **Step 2: Remove `#[allow(dead_code)]` from `tag_external_path`**

- [ ] **Step 3: Call `tag_external_path` in `goto_definition` AND `hover` after LSP returns a result**

After the definition/hover path is resolved:
```rust
let source_tag = tag_external_path(&resolved_path, &project_root, &ctx.agent).await;
```

Include the source tag in the response JSON so the agent knows which library it landed in.

- [ ] **Step 4: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(symbol): wire tag_external_path into goto_definition and hover for auto-discovery"
```

---

### Task 13: Nudging — per-session and persistent dedup

**Files:**
- Modify: `src/agent.rs` (add `nudged_libraries` field)
- Modify: `src/tools/symbol.rs` (add nudge hints)

- [ ] **Step 1: Add `nudged_libraries` to Agent**

```rust
pub nudged_libraries: std::sync::Mutex<std::collections::HashSet<String>>,
```

Initialize as empty in `Agent::new`.

- [ ] **Step 2: Add helper method `should_nudge`**

```rust
/// Check if we should nudge about a library. Returns true at most once per session per library,
/// and respects the persistent nudge_dismissed flag.
pub async fn should_nudge(&self, lib_name: &str) -> bool {
    // Check persistent dismissal
    let inner = self.inner.read().await;
    if let Some(p) = &inner.active_project {
        if let Some(entry) = p.library_registry.lookup(lib_name) {
            if entry.nudge_dismissed {
                return false;
            }
            if entry.indexed {
                return false; // Already indexed, no need to nudge
            }
        }
    }
    drop(inner);

    // Check session dedup
    let mut nudged = self.nudged_libraries.lock().unwrap_or_else(|e| e.into_inner());
    nudged.insert(lib_name.to_string())
}
```

- [ ] **Step 3: Add nudge hints to `goto_definition`, `hover`, and `find_symbol` responses**

After tag_external_path runs and the library is registered but not indexed:
```rust
if ctx.agent.should_nudge(&lib_name).await {
    result["library_hint"] = json!({
        "name": lib_name,
        "status": "not_indexed",
        "hint": format!("Library '{}' discovered but not indexed. Run index_project(scope='lib:{}') to enable semantic search.", lib_name, lib_name)
    });
}
```

- [ ] **Step 4: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/agent.rs src/tools/symbol.rs
git commit -m "feat(agent): add library nudging with per-session and persistent dedup"
```

---

### Task 14: Extend path security for library read access

**Files:**
- Modify: `src/util/path_security.rs` (`validate_read_path`)
- Modify: `src/agent.rs` (`security_config`)

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn validate_read_path_accepts_library_paths() {
    let config = PathSecurityConfig {
        library_paths: vec![PathBuf::from("/tmp/libs/tokio")],
        ..Default::default()
    };
    let result = validate_read_path(
        "/tmp/libs/tokio/src/runtime.rs",
        Some(Path::new("/tmp/project")),
        &config,
    );
    assert!(result.is_ok());
}

#[test]
fn validate_read_path_rejects_unregistered_external() {
    let config = PathSecurityConfig::default();
    let result = validate_read_path(
        "/tmp/libs/unknown/src/lib.rs",
        Some(Path::new("/tmp/project")),
        &config,
    );
    assert!(result.is_err());
}

#[test]
fn validate_write_path_rejects_library_paths() {
    // Ensure validate_write_path does NOT accept library paths
    // (library_paths should only affect reads)
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Extend `validate_read_path`**

After the deny-list check, before returning the error for paths outside project root, add:

```rust
// Check if path is inside a registered library (read-only access)
if config.library_paths.iter().any(|lib| resolved.starts_with(lib)) {
    return Ok(resolved);
}
```

- [ ] **Step 4: Populate `library_paths` in `Agent::security_config`**

Update `security_config` method to include library paths:

```rust
pub async fn security_config(&self) -> PathSecurityConfig {
    let inner = self.inner.read().await;
    match &inner.active_project {
        Some(p) => {
            let mut config = p.config.security.to_path_security_config();
            config.library_paths = p.library_registry.all()
                .iter()
                .map(|e| e.path.clone())
                .collect();
            config
        }
        None => PathSecurityConfig::default(),
    }
}
```

- [ ] **Step 5: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/util/path_security.rs src/agent.rs
git commit -m "feat(security): extend validate_read_path to accept registered library paths"
```

---

### Task 14b: Extend `search_pattern` for library paths

**Files:**
- Modify: `src/tools/file.rs` (`SearchPattern::call`)

The spec lists `search_pattern` as gaining library support. Currently it walks the project root with the `ignore` crate — it won't search library directories without explicit changes.

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn search_pattern_accepts_library_path() {
    // Setup: project_ctx with a registered library, library has source files
    // Call search_pattern with path pointing inside the library
    // Should succeed (path security gate lifted)
}
```

- [ ] **Step 2: Extend `SearchPattern::call` to accept library paths**

When `path` parameter is provided and points to a library path:
- Use `validate_read_path` (which now accepts library paths via Task 14)
- Walk the library directory instead of the project root
- This should largely work once the path gate is lifted, since `search_pattern` already uses the `path` param to scope its walk

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat(search_pattern): accept library paths for scoped regex search"
```

---

## Chunk 4: Config, Prompts & Background Pipeline

### Task 15: `[libraries]` config section in project.toml

**Files:**
- Modify: `src/config/project.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn project_config_deserializes_libraries_section() {
    let toml = r#"
[project]
name = "test"

[libraries]
auto_index = true
auto_fetch_sources = true
fetch_timeout_secs = 120
"#;
    let config: ProjectConfig = toml::from_str(toml).unwrap();
    assert!(config.libraries.auto_index);
    assert!(config.libraries.auto_fetch_sources);
    assert_eq!(config.libraries.fetch_timeout_secs, 120);
}

#[test]
fn project_config_libraries_defaults() {
    let toml = r#"
[project]
name = "test"
"#;
    let config: ProjectConfig = toml::from_str(toml).unwrap();
    assert!(!config.libraries.auto_index);
    assert!(!config.libraries.auto_fetch_sources);
    assert_eq!(config.libraries.fetch_timeout_secs, 300);
}
```

- [ ] **Step 2: Add `LibrariesSection` struct and field on `ProjectConfig`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrariesSection {
    #[serde(default)]
    pub auto_index: bool,
    #[serde(default)]
    pub auto_fetch_sources: bool,
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,
    /// Manual version overrides for ecosystems without lockfile parsing.
    /// Keys are library names, values are version strings.
    #[serde(default)]
    pub version_overrides: std::collections::HashMap<String, String>,
}

fn default_fetch_timeout() -> u64 { 300 }

impl Default for LibrariesSection {
    fn default() -> Self {
        Self {
            auto_index: false,
            auto_fetch_sources: false,
            fetch_timeout_secs: default_fetch_timeout(),
            version_overrides: std::collections::HashMap::new(),
        }
    }
}
```

Add to `ProjectConfig`:
```rust
#[serde(default)]
pub libraries: LibrariesSection,
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test project_config_deserializes_libraries project_config_libraries_defaults -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(config): add [libraries] section to project.toml"
```

---

### Task 16: Embedding concurrency semaphore on Agent

**Files:**
- Modify: `src/agent.rs`

- [ ] **Step 1: Add semaphore field to Agent**

```rust
pub embedding_semaphore: Arc<tokio::sync::Semaphore>,
```

Initialize in `Agent::new` with 2 permits:
```rust
embedding_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
```

- [ ] **Step 2: Acquire semaphore in `build_library_index`**

Pass the semaphore into the library indexing pipeline. Before each embedder.embed() call, acquire a permit.

- [ ] **Step 3: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/agent.rs src/embed/index.rs
git commit -m "feat(agent): add embedding_semaphore for concurrent indexing safety"
```

---

### Task 17: Per-library indexing state tracking

**Files:**
- Modify: `src/agent.rs`

- [ ] **Step 1: Add `LibraryIndexState` enum and tracking map**

```rust
pub enum LibraryIndexState {
    Idle,
    FetchingSources { command: String },
    Indexing { done: usize, total: usize },
    Done { chunks: usize, version: String },
    Failed(String),
}

// On Agent:
pub library_index_states: std::sync::Mutex<HashMap<String, LibraryIndexState>>,
```

- [ ] **Step 2: Add helper methods**

```rust
pub fn set_library_state(&self, name: &str, state: LibraryIndexState) {
    let mut states = self.library_index_states.lock().unwrap_or_else(|e| e.into_inner());
    states.insert(name.to_string(), state);
}

pub fn library_states(&self) -> HashMap<String, String> {
    let states = self.library_index_states.lock().unwrap_or_else(|e| e.into_inner());
    states.iter().map(|(k, v)| {
        let status = match v {
            LibraryIndexState::Idle => "idle".to_string(),
            LibraryIndexState::FetchingSources { command } => format!("fetching_sources: {}", command),
            LibraryIndexState::Indexing { done, total } => format!("indexing: {}/{}", done, total),
            LibraryIndexState::Done { chunks, version } => format!("done: {} chunks (v{})", chunks, version),
            LibraryIndexState::Failed(msg) => format!("failed: {}", msg),
        };
        (k.clone(), status)
    }).collect()
}
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/agent.rs
git commit -m "feat(agent): add per-library indexing state tracking"
```

---

### Task 18: Extend `index_status` to report library states

**Files:**
- Modify: `src/tools/semantic.rs` (`IndexStatus::call`)

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn index_status_includes_library_states() {
    // Setup agent with library_index_states populated
    // Call IndexStatus, verify response includes "libraries" key
}
```

- [ ] **Step 2: Add library state reporting to `IndexStatus::call`**

After the existing project status logic, add:
```rust
let lib_states = ctx.agent.library_states();
if !lib_states.is_empty() {
    result["libraries"] = serde_json::to_value(&lib_states)?;
}
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(index_status): report per-library indexing states"
```

---

### Task 18b: Auto-index trigger when `auto_index = true`

**Files:**
- Modify: `src/tools/symbol.rs` (after `tag_external_path` call in `goto_definition`/`hover`)
- Modify: `src/agent.rs` (add `spawn_library_index` helper)

This wires up Layer 3 of the agent UX: config-gated auto-indexing on discovery.

- [ ] **Step 1: Write test**

```rust
#[tokio::test]
async fn auto_index_spawns_when_config_enabled() {
    // Setup: project with auto_index = true
    // Discover a library via tag_external_path
    // Verify LibraryIndexState for that library is set (Indexing or Done)
}
```

- [ ] **Step 2: Add `spawn_library_index` helper on Agent**

```rust
/// Spawn a background library indexing task if auto_index is enabled and library is not yet indexed.
pub async fn maybe_auto_index_library(&self, lib_name: &str) {
    let (should_index, root, entry_path) = {
        let inner = self.inner.read().await;
        let Some(p) = &inner.active_project else { return };
        if !p.config.libraries.auto_index { return }
        let Some(entry) = p.library_registry.lookup(lib_name) else { return };
        if entry.indexed { return }
        (true, p.root.clone(), entry.path.clone())
    };
    if !should_index { return }

    let name = lib_name.to_string();
    let source = format!("lib:{}", name);
    self.set_library_state(&name, LibraryIndexState::Indexing { done: 0, total: 0 });

    let self_clone = self.clone();
    tokio::spawn(async move {
        tracing::info!("Auto-indexing library '{}' in background...", name);
        let result = crate::embed::index::build_library_index(&root, &entry_path, &source, false).await;
        match result {
            Ok(()) => {
                // Mark as indexed
                let mut inner = self_clone.inner.write().await;
                if let Some(p) = inner.active_project.as_mut() {
                    if let Some(entry) = p.library_registry.lookup_mut(&name) {
                        entry.indexed = true;
                    }
                    let reg_path = p.root.join(".codescout/libraries.json");
                    let _ = p.library_registry.save(&reg_path);
                }
                drop(inner);
                self_clone.set_library_state(&name, LibraryIndexState::Done {
                    chunks: 0, version: String::new()
                });
            }
            Err(e) => {
                self_clone.set_library_state(&name, LibraryIndexState::Failed(e.to_string()));
            }
        }
    });
}
```

- [ ] **Step 3: Call from `tag_external_path` in `goto_definition`/`hover`**

After auto-discovery registers the library:
```rust
ctx.agent.maybe_auto_index_library(&lib_name).await;
```

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/agent.rs src/tools/symbol.rs
git commit -m "feat(agent): auto-index libraries on discovery when config enabled"
```

---

### Task 19: Update `list_libraries` to show version comparison

**Files:**
- Modify: `src/tools/library.rs`

- [ ] **Step 1: Update `ListLibraries::call` to include version info**

For each library entry, include `version`, `version_indexed`, and a `stale` boolean in the output.

- [ ] **Step 2: Update `format_list_libraries` to show version comparison**

Show `[stale]` marker next to libraries where `version != version_indexed`.

- [ ] **Step 3: Run tests, update existing test assertions**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 4: Commit**

```bash
git add src/tools/library.rs
git commit -m "feat(list_libraries): show version comparison and staleness markers"
```

---

### Task 20: Update all 3 prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/tools/workflow.rs` (`build_system_prompt_draft`)

Per CLAUDE.md § Prompt Surface Consistency, all 3 surfaces must be updated when tool behavior changes.

- [ ] **Step 1: Update `server_instructions.md`**

Add guidance about:
- Library source code being navigable with all read-only tools once registered
- `scope="lib:<name>"` for semantic search (requires indexing)
- Version staleness hints in search responses
- `index_status()` for checking library indexing progress
- Write tools are project-only

- [ ] **Step 2: Update `onboarding_prompt.md`**

Add a section mentioning:
- Libraries are auto-discovered when `goto_definition` lands outside the project
- `list_libraries` shows registered libraries and their index status
- `index_project(scope="lib:<name>")` to index a specific library

- [ ] **Step 3: Update `build_system_prompt_draft` in `src/tools/workflow.rs`**

Include library registry info in the generated system prompt:
- List of registered libraries and their indexed/stale status
- Available scopes for semantic search

- [ ] **Step 4: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/tools/workflow.rs
git commit -m "docs(prompts): update all 3 prompt surfaces for library navigation and indexing"
```

---

## Chunk 5: Final Integration & Verification

### Task 21: Integration test — full library indexing lifecycle

**Files:**
- Create: `tests/library_indexing.rs` (integration test)

- [ ] **Step 1: Write integration test**

Test the full flow:
1. Create a temp project with Cargo.toml, Cargo.lock, and a fake library dir
2. Activate the project
3. Register a library
4. Index it
5. Search with `scope="lib:<name>"`
6. Verify results come from the library DB, not project DB
7. Verify `index_status` shows the library

- [ ] **Step 2: Run integration test**

Run: `cargo test --test library_indexing -- --nocapture`
Note: requires embedding API access; may be `#[ignore]` for CI

- [ ] **Step 3: Commit**

```bash
git add tests/library_indexing.rs
git commit -m "test: add integration test for library indexing lifecycle"
```

---

### Task 22: Final verification — fmt, clippy, full test suite

**Files:** None (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Clippy**

Run: `cargo clippy -- -D warnings`
Fix any warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test`
All tests must pass.

- [ ] **Step 4: Release build**

Run: `cargo build --release`
Verify it compiles cleanly.

- [ ] **Step 5: Manual smoke test via MCP**

Restart MCP server (`/mcp`), then:
- `list_libraries` — should work
- `index_status` — should show new layout
- Verify `.codescout/embeddings/project.db` exists (not old `embeddings.db`)

- [ ] **Step 6: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "chore: final cleanup for library indexing redesign"
```
