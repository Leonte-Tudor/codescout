# Project Dashboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `cargo run -- dashboard` CLI subcommand that launches a lightweight axum HTTP server serving a browser-based project health dashboard.

**Architecture:** A new `src/dashboard/` module starts an axum server on a configurable port. JSON API endpoints call existing library functions (`ProjectConfig::load_or_default`, `embed::index::index_stats`, `usage::db::query_stats`, `MemoryStore`, `LibraryRegistry`). Static HTML/CSS/JS is embedded in the binary via `include_str!`. Three phases: project overview, tool usage stats, memory/library management.

**Tech Stack:** `axum` 0.8 (HTTP framework), `tower-http` 0.6 (CORS), `open` 5 (browser launch), `serde_json` (already present), `rusqlite` (already present).

---

### Task 1: Add dashboard dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Context:** New deps go behind a `dashboard` feature flag (default: on). `axum` 0.8 is Tokio-native. `tower-http` provides CORS middleware. `open` launches the default browser. All three are only compiled when the feature is enabled.

**Step 1: Add dependencies and feature**

Add to the `[dependencies]` section of `Cargo.toml`, after the `libc` line:

```toml
# Dashboard web server (behind "dashboard" feature)
axum = { version = "0.8", features = ["json"], optional = true }
tower-http = { version = "0.6", features = ["cors"], optional = true }
open = { version = "5", optional = true }
```

Add to the `[features]` section, updating the `default` line and adding:

```toml
default = ["remote-embed", "dashboard"]
dashboard = ["dep:axum", "dep:tower-http", "dep:open"]
```

**Step 2: Verify it compiles**

```bash
cargo check
```
Expected: clean compile (no code uses the new deps yet).

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add axum, tower-http, open deps behind dashboard feature"
```

---

### Task 2: Dashboard module scaffold + CLI subcommand

**Files:**
- Create: `src/dashboard/mod.rs`
- Modify: `src/lib.rs` (add `pub mod dashboard;`)
- Modify: `src/main.rs` (add `Dashboard` variant to `Commands`)

**Context:** The `Commands` enum in `main.rs:16-51` has `Start` and `Index` variants. Add a `Dashboard` variant with `--project`, `--host`, `--port`, `--no-open` args. The module just exports a `pub async fn serve()` stub for now.

**Step 1: Create the module stub**

Create `src/dashboard/mod.rs`:

```rust
#[cfg(feature = "dashboard")]
mod routes;

use std::path::{Path, PathBuf};
use anyhow::Result;

/// Launch the dashboard HTTP server.
///
/// Reads project data from `.code-explorer/` and serves a web UI.
/// Does NOT start the MCP server, LSP, or tool machinery.
#[cfg(feature = "dashboard")]
pub async fn serve(
    project_root: PathBuf,
    host: String,
    port: u16,
    open_browser: bool,
) -> Result<()> {
    let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("Dashboard server starting at http://{}", addr);

    let router = routes::build_router(&project_root)?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    eprintln!("Dashboard: http://{}", actual_addr);

    if open_browser {
        let url = format!("http://{}", actual_addr);
        if let Err(e) = open::that(&url) {
            tracing::warn!("Failed to open browser: {}", e);
        }
    }

    axum::serve(listener, router)
        .with_graceful_shutdown(crate::server::shutdown_signal())
        .await?;

    Ok(())
}
```

**Step 2: Create the routes stub**

Create `src/dashboard/routes.rs`:

```rust
use std::path::{Path, PathBuf};
use axum::{Router, Json, routing::get};
use anyhow::Result;

/// Shared state passed to all handlers via axum State extractor.
#[derive(Clone)]
pub struct DashboardState {
    pub project_root: PathBuf,
}

pub fn build_router(project_root: &Path) -> Result<Router> {
    let state = DashboardState {
        project_root: project_root.to_path_buf(),
    };

    let router = Router::new()
        .route("/api/health", get(health))
        .with_state(state);

    Ok(router)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
```

**Step 3: Add module to lib.rs**

Add to `src/lib.rs` after the `config` module declaration:

```rust
#[cfg(feature = "dashboard")]
pub mod dashboard;
```

**Step 4: Add CLI subcommand to main.rs**

Add a `Dashboard` variant to the `Commands` enum (after `Index`):

```rust
    /// Launch the project dashboard web UI
    #[cfg(feature = "dashboard")]
    Dashboard {
        /// Project root path (defaults to CWD)
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,

        /// Listen address
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Listen port
        #[arg(long, default_value_t = 8099)]
        port: u16,

        /// Don't auto-open the browser
        #[arg(long)]
        no_open: bool,
    },
```

Add the match arm in `main()` (after the `Index` arm):

```rust
        #[cfg(feature = "dashboard")]
        Commands::Dashboard {
            project,
            host,
            port,
            no_open,
        } => {
            let root = project
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            tracing::info!("Launching dashboard for {}", root.display());
            code_explorer::dashboard::serve(root, host, port, !no_open).await?;
        }
```

**Step 5: Make `shutdown_signal` public**

In `src/server.rs`, change `async fn shutdown_signal()` to `pub async fn shutdown_signal()` (it's currently `pub(crate)` or private — the dashboard needs it for graceful shutdown).

**Step 6: Verify it compiles**

```bash
cargo check
```
Expected: clean compile.

**Step 7: Commit**

```bash
git add src/dashboard/ src/lib.rs src/main.rs src/server.rs
git commit -m "feat(dashboard): scaffold module + CLI subcommand"
```

---

### Task 3: Phase 1 API — project info endpoint

**Files:**
- Create: `src/dashboard/api/mod.rs`
- Create: `src/dashboard/api/project.rs`
- Modify: `src/dashboard/routes.rs` (add route)
- Modify: `src/dashboard/mod.rs` (add `mod api;`)

**Context:** The `/api/project` endpoint returns project name, root path, detected languages, and git info. Language detection uses `ast::detect_language()` on files found by walking the project. Git info comes from `git2::Repository::open()`. `ProjectConfig::load_or_default()` gives us the project name.

**Step 1: Write the failing test**

Add to `src/dashboard/routes.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_router(root: &std::path::Path) -> Router {
        build_router(root).unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn project_info_returns_root() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/project")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["root"].as_str().is_some());
    }
}
```

**Step 2: Run to verify they fail**

```bash
cargo test dashboard::routes::tests 2>&1 | head -20
```
Expected: `project_info_returns_root` fails — route not registered yet.

**Step 3: Implement**

Create `src/dashboard/api/mod.rs`:

```rust
pub mod project;
```

Create `src/dashboard/api/project.rs`:

```rust
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use crate::config::project::ProjectConfig;
use super::super::routes::DashboardState;

pub async fn get_project_info(State(state): State<DashboardState>) -> Json<Value> {
    let root = &state.project_root;
    let config = ProjectConfig::load_or_default(root);
    let name = config.project.name.clone();

    // Detect languages by scanning file extensions
    let languages = detect_languages(root);

    // Git info
    let (git_branch, git_dirty) = git_info(root);

    Json(json!({
        "name": name,
        "root": root.display().to_string(),
        "languages": languages,
        "git_branch": git_branch,
        "git_dirty": git_dirty,
    }))
}

fn detect_languages(root: &std::path::Path) -> Vec<String> {
    let mut langs = std::collections::BTreeSet::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .take(500)
    {
        if let Some(lang) = crate::ast::detect_language(entry.path()) {
            langs.insert(lang.to_string());
        }
    }
    langs.into_iter().collect()
}

fn git_info(root: &std::path::Path) -> (Option<String>, bool) {
    match crate::git::open_repo(root) {
        Ok(repo) => {
            let branch = repo
                .head()
                .ok()
                .and_then(|h| h.shorthand().map(String::from));
            let dirty = repo
                .statuses(None)
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            (branch, dirty)
        }
        Err(_) => (None, false),
    }
}
```

Add to `src/dashboard/mod.rs` (inside the `#[cfg(feature = "dashboard")]` block, before `pub async fn serve`):

```rust
mod api;
```

Update `src/dashboard/routes.rs` to add the route:

```rust
use super::api;

// In build_router, add before .with_state:
    .route("/api/project", get(api::project::get_project_info))
```

**Step 4: Run tests**

```bash
cargo test dashboard::routes::tests
```
Expected: both tests pass.

**Step 5: Commit**

```bash
cargo clippy -- -D warnings && cargo fmt
git add src/dashboard/
git commit -m "feat(dashboard): add /api/project endpoint with lang detection + git info"
```

---

### Task 4: Phase 1 API — config + index + drift endpoints

**Files:**
- Create: `src/dashboard/api/config.rs`
- Create: `src/dashboard/api/index.rs`
- Modify: `src/dashboard/api/mod.rs`
- Modify: `src/dashboard/routes.rs`

**Context:** These endpoints call existing library functions:
- `/api/config` → `ProjectConfig::load_or_default(root)` → serialize to JSON
- `/api/index` → `embed::index::open_db()` + `index_stats()` + `check_index_staleness()`
- `/api/drift?threshold=0.1` → `embed::index::query_drift_report()`

The pattern is the same for all: extract `State<DashboardState>`, call library function, wrap in `Json()`.

**Step 1: Write failing tests**

Add to `src/dashboard/routes.rs` tests:

```rust
    #[tokio::test]
    async fn config_returns_json() {
        let dir = tempfile::TempDir::new().unwrap();
        // Create minimal project.toml so config loads
        let ce_dir = dir.path().join(".code-explorer");
        std::fs::create_dir_all(&ce_dir).unwrap();
        std::fs::write(
            ce_dir.join("project.toml"),
            "[project]\nname = \"test-project\"\n",
        ).unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/config").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["project"]["name"], "test-project");
    }

    #[tokio::test]
    async fn index_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/index").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }
```

**Step 2: Run to verify they fail**

```bash
cargo test dashboard::routes::tests 2>&1 | head -20
```
Expected: fail — routes not registered.

**Step 3: Implement**

`src/dashboard/api/config.rs`:

```rust
use axum::extract::State;
use axum::Json;
use serde_json::Value;
use crate::config::project::ProjectConfig;
use super::super::routes::DashboardState;

pub async fn get_config(State(state): State<DashboardState>) -> Json<Value> {
    let config = ProjectConfig::load_or_default(&state.project_root);
    // ProjectConfig derives Serialize
    Json(serde_json::to_value(config).unwrap_or_default())
}
```

`src/dashboard/api/index.rs`:

```rust
use axum::extract::{State, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::embed::index as embed_index;
use super::super::routes::DashboardState;

pub async fn get_index(State(state): State<DashboardState>) -> Json<Value> {
    let db_path = state.project_root.join(".code-explorer").join("embeddings.db");
    if !db_path.exists() {
        return Json(json!({
            "available": false,
            "reason": "No semantic index. Run `code-explorer index` to build one."
        }));
    }

    let conn = match embed_index::open_db(&state.project_root) {
        Ok(c) => c,
        Err(e) => {
            return Json(json!({
                "available": false,
                "reason": format!("Failed to open index DB: {}", e)
            }));
        }
    };

    let stats = embed_index::index_stats(&conn).unwrap_or(embed_index::IndexStats {
        file_count: 0,
        chunk_count: 0,
        embedding_count: 0,
        model: None,
    });

    let staleness = embed_index::check_index_staleness(&conn, &state.project_root)
        .unwrap_or(embed_index::Staleness {
            stale: true,
            behind_commits: 0,
        });

    Json(json!({
        "available": true,
        "file_count": stats.file_count,
        "chunk_count": stats.chunk_count,
        "embedding_count": stats.embedding_count,
        "model": stats.model,
        "stale": staleness.stale,
        "behind_commits": staleness.behind_commits,
    }))
}

#[derive(Deserialize)]
pub struct DriftParams {
    pub threshold: Option<f32>,
}

pub async fn get_drift(
    State(state): State<DashboardState>,
    Query(params): Query<DriftParams>,
) -> Json<Value> {
    let db_path = state.project_root.join(".code-explorer").join("embeddings.db");
    if !db_path.exists() {
        return Json(json!({ "available": false, "files": [] }));
    }

    let conn = match embed_index::open_db(&state.project_root) {
        Ok(c) => c,
        Err(_) => return Json(json!({ "available": false, "files": [] })),
    };

    let threshold = params.threshold.unwrap_or(0.1);
    let rows = embed_index::query_drift_report(&conn, Some(threshold), None)
        .unwrap_or_default();

    let files: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "path": r.file_path,
                "avg_drift": r.avg_drift,
                "max_drift": r.max_drift,
                "chunks_added": r.chunks_added,
                "chunks_removed": r.chunks_removed,
            })
        })
        .collect();

    Json(json!({
        "available": true,
        "threshold": threshold,
        "files": files,
    }))
}
```

Update `src/dashboard/api/mod.rs`:

```rust
pub mod config;
pub mod index;
pub mod project;
```

Update `src/dashboard/routes.rs` — add routes:

```rust
    .route("/api/config", get(api::config::get_config))
    .route("/api/index", get(api::index::get_index))
    .route("/api/drift", get(api::index::get_drift))
```

**Step 4: Run tests**

```bash
cargo test dashboard::routes::tests
```
Expected: all 4 tests pass.

**Step 5: Clippy + format + commit**

```bash
cargo clippy -- -D warnings && cargo fmt
git add src/dashboard/
git commit -m "feat(dashboard): add /api/config, /api/index, /api/drift endpoints"
```

---

### Task 5: Phase 2 API — usage + errors endpoints

**Files:**
- Create: `src/dashboard/api/usage.rs`
- Create: `src/dashboard/api/errors.rs`
- Modify: `src/usage/db.rs` (add `recent_errors()` query)
- Modify: `src/dashboard/api/mod.rs`
- Modify: `src/dashboard/routes.rs`

**Context:** `/api/usage` calls `usage::db::query_stats()` — same as the `get_usage_stats` MCP tool. `/api/errors` needs a new `recent_errors()` function in `usage/db.rs` that queries the most recent error rows. The `usage.db` might not exist if the MCP server hasn't run yet — graceful degradation.

**Step 1: Write the failing test for `recent_errors`**

Add to `src/usage/db.rs` test module:

```rust
    #[test]
    fn recent_errors_returns_latest_errors() {
        let (_dir, conn) = tmp();
        write_record(&conn, "find_symbol", 50, "success", false, None).unwrap();
        write_record(&conn, "semantic_search", 100, "error", false, Some("index missing")).unwrap();
        write_record(&conn, "list_symbols", 30, "recoverable_error", false, Some("path not found")).unwrap();

        let errors = recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 2);
        // Most recent first
        assert_eq!(errors[0].tool, "list_symbols");
        assert_eq!(errors[1].tool, "semantic_search");
    }

    #[test]
    fn recent_errors_respects_limit() {
        let (_dir, conn) = tmp();
        for i in 0..5 {
            write_record(&conn, &format!("tool_{}", i), 10, "error", false, Some("fail")).unwrap();
        }
        let errors = recent_errors(&conn, 3).unwrap();
        assert_eq!(errors.len(), 3);
    }
```

**Step 2: Run to verify they fail**

```bash
cargo test usage::db::tests::recent_errors 2>&1 | head -10
```
Expected: compile error — `recent_errors` not defined.

**Step 3: Implement `recent_errors` in `src/usage/db.rs`**

Add after `query_stats`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct ErrorRecord {
    pub tool: String,
    pub timestamp: String,
    pub outcome: String,
    pub message: Option<String>,
}

pub fn recent_errors(conn: &Connection, limit: i64) -> Result<Vec<ErrorRecord>> {
    let mut stmt = conn.prepare(
        "SELECT tool_name, called_at, outcome, error_msg
         FROM tool_calls
         WHERE outcome IN ('error', 'recoverable_error')
         ORDER BY called_at DESC
         LIMIT ?",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(ErrorRecord {
                tool: r.get(0)?,
                timestamp: r.get(1)?,
                outcome: r.get(2)?,
                message: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

**Step 4: Run DB tests**

```bash
cargo test usage::db::tests
```
Expected: all pass (including the 2 new ones).

**Step 5: Implement dashboard API endpoints**

`src/dashboard/api/usage.rs`:

```rust
use axum::extract::{State, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use super::super::routes::DashboardState;

#[derive(Deserialize)]
pub struct UsageParams {
    pub window: Option<String>,
}

pub async fn get_usage(
    State(state): State<DashboardState>,
    Query(params): Query<UsageParams>,
) -> Json<Value> {
    let db_path = state.project_root.join(".code-explorer").join("usage.db");
    if !db_path.exists() {
        return Json(json!({
            "available": false,
            "reason": "No usage data. Tool statistics are recorded when the MCP server runs."
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
    match crate::usage::db::query_stats(&conn, window) {
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

`src/dashboard/api/errors.rs`:

```rust
use axum::extract::{State, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use super::super::routes::DashboardState;

#[derive(Deserialize)]
pub struct ErrorParams {
    pub limit: Option<i64>,
}

pub async fn get_errors(
    State(state): State<DashboardState>,
    Query(params): Query<ErrorParams>,
) -> Json<Value> {
    let db_path = state.project_root.join(".code-explorer").join("usage.db");
    if !db_path.exists() {
        return Json(json!({ "available": false, "errors": [] }));
    }

    let conn = match crate::usage::db::open_db(&state.project_root) {
        Ok(c) => c,
        Err(_) => return Json(json!({ "available": false, "errors": [] })),
    };

    let limit = params.limit.unwrap_or(20);
    let errors = crate::usage::db::recent_errors(&conn, limit).unwrap_or_default();
    Json(json!({
        "available": true,
        "errors": errors,
    }))
}
```

Update `src/dashboard/api/mod.rs`:

```rust
pub mod config;
pub mod errors;
pub mod index;
pub mod project;
pub mod usage;
```

Update `src/dashboard/routes.rs` — add routes:

```rust
    .route("/api/usage", get(api::usage::get_usage))
    .route("/api/errors", get(api::errors::get_errors))
```

**Step 6: Add dashboard route tests for usage/errors**

Add to `src/dashboard/routes.rs` tests:

```rust
    #[tokio::test]
    async fn usage_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/usage").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }

    #[tokio::test]
    async fn errors_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/errors").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }
```

**Step 7: Run all dashboard tests**

```bash
cargo test dashboard::routes::tests && cargo test usage::db::tests
```
Expected: all pass.

**Step 8: Clippy + format + commit**

```bash
cargo clippy -- -D warnings && cargo fmt
git add src/dashboard/ src/usage/db.rs
git commit -m "feat(dashboard): add /api/usage and /api/errors endpoints, add recent_errors query"
```

---

### Task 6: Phase 3 API — memories + libraries endpoints

**Files:**
- Create: `src/dashboard/api/memories.rs`
- Create: `src/dashboard/api/libraries.rs`
- Modify: `src/dashboard/api/mod.rs`
- Modify: `src/dashboard/routes.rs`

**Context:** Memories use `memory::MemoryStore` which has `open()`, `list()`, `read()`, `write()`, `delete()`. Libraries use `library::registry::LibraryRegistry::load()` from `.code-explorer/libraries.json`. Both are filesystem-based, not SQLite.

**Step 1: Write failing tests**

Add to `src/dashboard/routes.rs` tests:

```rust
    #[tokio::test]
    async fn memories_list_returns_empty_for_fresh_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/memories").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["topics"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn libraries_returns_empty_for_fresh_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder().uri("/api/libraries").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["libraries"].as_array().unwrap().is_empty());
    }
```

**Step 2: Run to verify they fail**

```bash
cargo test dashboard::routes::tests 2>&1 | head -20
```

**Step 3: Implement**

`src/dashboard/api/memories.rs`:

```rust
use axum::extract::{State, Path};
use axum::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::memory::MemoryStore;
use super::super::routes::DashboardState;

pub async fn list_memories(State(state): State<DashboardState>) -> Json<Value> {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(_) => return Json(json!({ "topics": [] })),
    };
    let topics = store.list().unwrap_or_default();
    Json(json!({ "topics": topics }))
}

pub async fn get_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    };
    match store.read(&topic) {
        Ok(Some(content)) => (StatusCode::OK, Json(json!({ "topic": topic, "content": content }))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "Not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}

#[derive(Deserialize)]
pub struct WriteMemoryBody {
    pub content: String,
}

pub async fn write_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
    Json(body): Json<WriteMemoryBody>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    };
    match store.write(&topic, &body.content) {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}

pub async fn delete_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    };
    match store.delete(&topic) {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}
```

`src/dashboard/api/libraries.rs`:

```rust
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use crate::library::registry::LibraryRegistry;
use super::super::routes::DashboardState;

pub async fn get_libraries(State(state): State<DashboardState>) -> Json<Value> {
    let registry_path = state
        .project_root
        .join(".code-explorer")
        .join("libraries.json");
    let registry = LibraryRegistry::load(&registry_path).unwrap_or_else(|_| LibraryRegistry::new());

    let libs: Vec<Value> = registry
        .all()
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "path": e.path.display().to_string(),
                "language": e.language,
                "indexed": e.indexed,
                "version": e.version,
            })
        })
        .collect();

    Json(json!({ "libraries": libs }))
}
```

Update `src/dashboard/api/mod.rs`:

```rust
pub mod config;
pub mod errors;
pub mod index;
pub mod libraries;
pub mod memories;
pub mod project;
pub mod usage;
```

Update `src/dashboard/routes.rs` — add routes (note: memories needs `post` and `delete`):

```rust
use axum::routing::{get, post, delete};

// In build_router:
    .route("/api/memories", get(api::memories::list_memories))
    .route("/api/memories/{topic}", get(api::memories::get_memory))
    .route("/api/memories/{topic}", post(api::memories::write_memory))
    .route("/api/memories/{topic}", delete(api::memories::delete_memory))
    .route("/api/libraries", get(api::libraries::get_libraries))
```

**Note:** axum 0.8 uses `{param}` syntax for path parameters (not `:param`). If using axum 0.7, use `:topic` instead.

**Step 4: Run tests**

```bash
cargo test dashboard::routes::tests
```
Expected: all pass.

**Step 5: Clippy + format + commit**

```bash
cargo clippy -- -D warnings && cargo fmt
git add src/dashboard/
git commit -m "feat(dashboard): add /api/memories CRUD and /api/libraries endpoints"
```

---

### Task 7: Static frontend — HTML shell + CSS

**Files:**
- Create: `src/dashboard/static/index.html`
- Create: `src/dashboard/static/dashboard.css`
- Modify: `src/dashboard/routes.rs` (serve static files)

**Context:** The SPA shell loads `dashboard.css` and `dashboard.js`. For release builds, files are embedded with `include_str!`. For debug builds, read from filesystem for fast iteration. The HTML has three page containers: Overview, Tool Stats, Memories. Light/dark theme toggle. No frameworks.

**Step 1: Create the HTML shell**

`src/dashboard/static/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>code-explorer dashboard</title>
    <link rel="stylesheet" href="/dashboard.css">
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4"></script>
</head>
<body>
    <header>
        <h1>code-explorer</h1>
        <nav>
            <button class="nav-btn active" data-page="overview">Overview</button>
            <button class="nav-btn" data-page="stats">Tool Stats</button>
            <button class="nav-btn" data-page="memories">Memories</button>
        </nav>
        <button id="theme-toggle" title="Toggle theme">☀</button>
    </header>

    <main>
        <div id="page-overview" class="page active">
            <div class="card-grid">
                <div class="card" id="card-project">
                    <h2>Project</h2>
                    <div class="card-body" id="project-info">Loading...</div>
                </div>
                <div class="card" id="card-config">
                    <h2>Configuration</h2>
                    <div class="card-body" id="config-info">Loading...</div>
                </div>
                <div class="card" id="card-index">
                    <h2>Semantic Index</h2>
                    <div class="card-body" id="index-info">Loading...</div>
                </div>
                <div class="card" id="card-drift">
                    <h2>Drift</h2>
                    <div class="card-body" id="drift-info">Loading...</div>
                </div>
                <div class="card" id="card-libraries">
                    <h2>Libraries</h2>
                    <div class="card-body" id="libraries-info">Loading...</div>
                </div>
            </div>
        </div>

        <div id="page-stats" class="page">
            <div class="stats-controls">
                <label>Window:</label>
                <select id="stats-window">
                    <option value="1h">1 hour</option>
                    <option value="24h">24 hours</option>
                    <option value="7d">7 days</option>
                    <option value="30d" selected>30 days</option>
                </select>
            </div>
            <div class="card" id="card-usage-summary">
                <h2>Summary</h2>
                <div class="card-body" id="usage-summary">Loading...</div>
            </div>
            <div class="card" id="card-usage-chart">
                <h2>Calls by Tool</h2>
                <canvas id="calls-chart" height="300"></canvas>
            </div>
            <div class="card" id="card-usage-table">
                <h2>Per-Tool Breakdown</h2>
                <div class="card-body" id="usage-table">Loading...</div>
            </div>
            <div class="card" id="card-errors">
                <h2>Recent Errors</h2>
                <div class="card-body" id="errors-list">Loading...</div>
            </div>
        </div>

        <div id="page-memories" class="page">
            <div class="memories-layout">
                <div class="memories-sidebar">
                    <h2>Topics</h2>
                    <button id="new-memory-btn" class="btn">+ New</button>
                    <ul id="memory-topics"></ul>
                </div>
                <div class="memories-content">
                    <div id="memory-viewer">
                        <p class="muted">Select a topic to view its content.</p>
                    </div>
                </div>
            </div>
        </div>
    </main>

    <footer>
        <span id="last-refresh">Last refreshed: —</span>
    </footer>

    <script src="/dashboard.js"></script>
</body>
</html>
```

**Step 2: Create the CSS**

`src/dashboard/static/dashboard.css`:

```css
:root {
    --bg: #f8f9fa;
    --bg-card: #ffffff;
    --text: #212529;
    --text-muted: #6c757d;
    --border: #dee2e6;
    --accent: #0d6efd;
    --accent-hover: #0b5ed7;
    --success: #198754;
    --warning: #ffc107;
    --danger: #dc3545;
    --font: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    --mono: "SF Mono", "Fira Code", "Fira Mono", monospace;
    --radius: 8px;
}

[data-theme="dark"] {
    --bg: #1a1a2e;
    --bg-card: #16213e;
    --text: #e0e0e0;
    --text-muted: #8d8d8d;
    --border: #333;
    --accent: #4dabf7;
    --accent-hover: #339af0;
}

* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: var(--font); background: var(--bg); color: var(--text); }

header {
    display: flex; align-items: center; gap: 1rem;
    padding: 0.75rem 1.5rem;
    background: var(--bg-card); border-bottom: 1px solid var(--border);
}
header h1 { font-size: 1.1rem; font-weight: 600; }
header nav { display: flex; gap: 0.25rem; margin-left: 1rem; }
.nav-btn {
    padding: 0.4rem 0.8rem; border: 1px solid var(--border);
    border-radius: var(--radius); cursor: pointer;
    background: transparent; color: var(--text); font-size: 0.85rem;
}
.nav-btn.active { background: var(--accent); color: #fff; border-color: var(--accent); }
#theme-toggle {
    margin-left: auto; border: none; background: none;
    font-size: 1.2rem; cursor: pointer;
}

main { padding: 1.5rem; max-width: 1200px; margin: 0 auto; }
.page { display: none; }
.page.active { display: block; }

.card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr)); gap: 1rem; }
.card {
    background: var(--bg-card); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 1rem;
}
.card h2 { font-size: 0.9rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 0.75rem; }
.card-body { font-size: 0.9rem; line-height: 1.5; }

.status-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 0.4rem; }
.status-fresh { background: var(--success); }
.status-stale { background: var(--warning); }
.status-error { background: var(--danger); }

table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
th, td { padding: 0.4rem 0.6rem; text-align: left; border-bottom: 1px solid var(--border); }
th { font-weight: 600; color: var(--text-muted); }
td.num { text-align: right; font-family: var(--mono); }

.stats-controls { margin-bottom: 1rem; display: flex; align-items: center; gap: 0.5rem; }
.stats-controls select { padding: 0.3rem; border-radius: var(--radius); border: 1px solid var(--border); }

.memories-layout { display: grid; grid-template-columns: 220px 1fr; gap: 1rem; min-height: 400px; }
.memories-sidebar { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 1rem; }
.memories-sidebar ul { list-style: none; margin-top: 0.5rem; }
.memories-sidebar li { padding: 0.3rem 0.5rem; cursor: pointer; border-radius: 4px; font-size: 0.85rem; }
.memories-sidebar li:hover { background: var(--border); }
.memories-sidebar li.active { background: var(--accent); color: #fff; }
.memories-content { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 1rem; }
#memory-viewer pre { white-space: pre-wrap; font-family: var(--mono); font-size: 0.85rem; }

.btn { padding: 0.4rem 0.8rem; border: 1px solid var(--border); border-radius: var(--radius); cursor: pointer; background: var(--accent); color: #fff; font-size: 0.8rem; }
.btn:hover { background: var(--accent-hover); }
.btn-danger { background: var(--danger); }

.muted { color: var(--text-muted); font-style: italic; }

footer { padding: 0.5rem 1.5rem; font-size: 0.75rem; color: var(--text-muted); text-align: right; }
```

**Step 3: Add static file serving to routes.rs**

Add static file handlers to `src/dashboard/routes.rs`. Use `include_str!` for release, filesystem for debug:

```rust
use axum::response::Html;
use axum::http::header;

// Embed static files
#[cfg(not(debug_assertions))]
mod embedded {
    pub const INDEX_HTML: &str = include_str!("static/index.html");
    pub const DASHBOARD_CSS: &str = include_str!("static/dashboard.css");
    pub const DASHBOARD_JS: &str = include_str!("static/dashboard.js");
}

async fn serve_index() -> Html<String> {
    #[cfg(not(debug_assertions))]
    { Html(embedded::INDEX_HTML.to_string()) }
    #[cfg(debug_assertions)]
    {
        let content = std::fs::read_to_string("src/dashboard/static/index.html")
            .unwrap_or_else(|_| "Dashboard HTML not found".into());
        Html(content)
    }
}

async fn serve_css() -> ([(header::HeaderName, &'static str); 1], String) {
    #[cfg(not(debug_assertions))]
    let content = embedded::DASHBOARD_CSS.to_string();
    #[cfg(debug_assertions)]
    let content = std::fs::read_to_string("src/dashboard/static/dashboard.css")
        .unwrap_or_default();
    ([(header::CONTENT_TYPE, "text/css")], content)
}

async fn serve_js() -> ([(header::HeaderName, &'static str); 1], String) {
    #[cfg(not(debug_assertions))]
    let content = embedded::DASHBOARD_JS.to_string();
    #[cfg(debug_assertions)]
    let content = std::fs::read_to_string("src/dashboard/static/dashboard.js")
        .unwrap_or_default();
    ([(header::CONTENT_TYPE, "application/javascript")], content)
}

// Add to build_router, before .with_state:
    .route("/", get(serve_index))
    .route("/dashboard.css", get(serve_css))
    .route("/dashboard.js", get(serve_js))
```

**Step 4: Verify it compiles**

```bash
cargo check
```
Expected: may warn about missing `dashboard.js` — that's fine, we create it in the next task.

**Step 5: Commit**

```bash
cargo fmt
git add src/dashboard/
git commit -m "feat(dashboard): add HTML shell, CSS styling, static file serving"
```

---

### Task 8: Static frontend — JavaScript

**Files:**
- Create: `src/dashboard/static/dashboard.js`

**Context:** Vanilla JS, no framework. Polls all `/api/*` endpoints every 5 seconds. Renders data into DOM. Chart.js for the tool calls bar chart. Light/dark theme toggle saved to localStorage.

**Step 1: Create dashboard.js**

`src/dashboard/static/dashboard.js`:

```javascript
(function() {
    'use strict';

    const POLL_INTERVAL = 5000;
    let callsChart = null;

    // --- Page navigation ---
    document.querySelectorAll('.nav-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
            document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
            btn.classList.add('active');
            document.getElementById('page-' + btn.dataset.page).classList.add('active');
        });
    });

    // --- Theme ---
    const theme = localStorage.getItem('ce-theme') || 'light';
    if (theme === 'dark') document.documentElement.setAttribute('data-theme', 'dark');
    document.getElementById('theme-toggle').addEventListener('click', () => {
        const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
        document.documentElement.setAttribute('data-theme', isDark ? '' : 'dark');
        localStorage.setItem('ce-theme', isDark ? 'light' : 'dark');
        document.getElementById('theme-toggle').textContent = isDark ? '☀' : '☾';
    });
    document.getElementById('theme-toggle').textContent = theme === 'dark' ? '☾' : '☀';

    // --- Data fetching ---
    async function fetchJson(url) {
        try {
            const resp = await fetch(url);
            return await resp.json();
        } catch (e) {
            return null;
        }
    }

    // --- Render helpers ---
    function kv(label, value) { return `<div><strong>${label}:</strong> ${value}</div>`; }
    function dot(cls) { return `<span class="status-dot ${cls}"></span>`; }

    // --- Overview page ---
    async function refreshOverview() {
        const [proj, config, index, drift, libs] = await Promise.all([
            fetchJson('/api/project'),
            fetchJson('/api/config'),
            fetchJson('/api/index'),
            fetchJson('/api/drift?threshold=0.1'),
            fetchJson('/api/libraries'),
        ]);

        if (proj) {
            const langs = (proj.languages || []).join(', ') || 'none detected';
            const git = proj.git_branch
                ? `${proj.git_branch}${proj.git_dirty ? ' (dirty)' : ''}`
                : 'not a git repo';
            document.getElementById('project-info').innerHTML =
                kv('Name', proj.name) + kv('Root', `<code>${proj.root}</code>`) +
                kv('Languages', langs) + kv('Git', git);
        }

        if (config) {
            const embed = config.embeddings || {};
            const sec = config.security || {};
            document.getElementById('config-info').innerHTML =
                kv('Embedding model', embed.model || 'default') +
                kv('Chunk size', embed.chunk_size || '?') +
                kv('Shell mode', sec.shell_mode || 'disabled');
        }

        if (index) {
            if (!index.available) {
                document.getElementById('index-info').innerHTML =
                    `<p class="muted">${index.reason}</p>`;
            } else {
                const statusCls = index.stale ? 'status-stale' : 'status-fresh';
                const statusText = index.stale
                    ? `Stale (${index.behind_commits} commits behind)`
                    : 'Up to date';
                document.getElementById('index-info').innerHTML =
                    `${dot(statusCls)}${statusText}` +
                    kv('Files', index.file_count) +
                    kv('Chunks', index.chunk_count) +
                    kv('Model', index.model || 'unknown');
            }
        }

        if (drift && drift.available && drift.files && drift.files.length > 0) {
            const rows = drift.files.slice(0, 10).map(f =>
                `<tr><td>${f.path}</td><td class="num">${f.avg_drift.toFixed(2)}</td><td class="num">${f.max_drift.toFixed(2)}</td></tr>`
            ).join('');
            document.getElementById('drift-info').innerHTML =
                `<table><thead><tr><th>File</th><th>Avg</th><th>Max</th></tr></thead><tbody>${rows}</tbody></table>`;
        } else {
            document.getElementById('drift-info').innerHTML =
                '<p class="muted">No significant drift detected.</p>';
        }

        if (libs) {
            const entries = libs.libraries || [];
            if (entries.length === 0) {
                document.getElementById('libraries-info').innerHTML =
                    '<p class="muted">No libraries registered.</p>';
            } else {
                const rows = entries.map(l =>
                    `<tr><td>${l.name}</td><td>${l.language}</td><td>${l.indexed ? '✓' : '—'}</td></tr>`
                ).join('');
                document.getElementById('libraries-info').innerHTML =
                    `<table><thead><tr><th>Name</th><th>Language</th><th>Indexed</th></tr></thead><tbody>${rows}</tbody></table>`;
            }
        }
    }

    // --- Tool Stats page ---
    async function refreshStats() {
        const window = document.getElementById('stats-window').value;
        const [usage, errors] = await Promise.all([
            fetchJson(`/api/usage?window=${window}`),
            fetchJson('/api/errors?limit=20'),
        ]);

        if (usage && usage.available) {
            const tools = usage.by_tool || [];
            const totalCalls = usage.total_calls || 0;
            const totalErrors = tools.reduce((sum, t) => sum + t.errors, 0);
            const totalOverflows = tools.reduce((sum, t) => sum + t.overflows, 0);
            const errorPct = totalCalls > 0 ? (totalErrors / totalCalls * 100).toFixed(1) : '0';
            const overflowPct = totalCalls > 0 ? (totalOverflows / totalCalls * 100).toFixed(1) : '0';

            document.getElementById('usage-summary').innerHTML =
                `<strong>${totalCalls}</strong> total calls &nbsp;|&nbsp; ` +
                `<strong>${errorPct}%</strong> error rate &nbsp;|&nbsp; ` +
                `<strong>${overflowPct}%</strong> overflow rate`;

            // Chart
            const labels = tools.map(t => t.tool);
            const data = tools.map(t => t.calls);
            const ctx = document.getElementById('calls-chart').getContext('2d');
            if (callsChart) callsChart.destroy();
            callsChart = new Chart(ctx, {
                type: 'bar',
                data: {
                    labels,
                    datasets: [{
                        label: 'Calls',
                        data,
                        backgroundColor: 'rgba(13, 110, 253, 0.7)',
                    }],
                },
                options: {
                    responsive: true,
                    plugins: { legend: { display: false } },
                    scales: { y: { beginAtZero: true } },
                },
            });

            // Table
            if (tools.length > 0) {
                const thead = '<thead><tr><th>Tool</th><th>Calls</th><th>Errors</th><th>Err%</th><th>Overflows</th><th>Ovf%</th><th>p50</th><th>p99</th></tr></thead>';
                const rows = tools.map(t =>
                    `<tr><td>${t.tool}</td><td class="num">${t.calls}</td><td class="num">${t.errors}</td>` +
                    `<td class="num">${t.error_rate_pct.toFixed(1)}%</td><td class="num">${t.overflows}</td>` +
                    `<td class="num">${t.overflow_rate_pct.toFixed(1)}%</td><td class="num">${t.p50_ms}ms</td>` +
                    `<td class="num">${t.p99_ms}ms</td></tr>`
                ).join('');
                document.getElementById('usage-table').innerHTML = `<table>${thead}<tbody>${rows}</tbody></table>`;
            } else {
                document.getElementById('usage-table').innerHTML = '<p class="muted">No tool calls in this window.</p>';
            }
        } else {
            const reason = (usage && usage.reason) || 'No usage data available.';
            document.getElementById('usage-summary').innerHTML = `<p class="muted">${reason}</p>`;
            document.getElementById('usage-table').innerHTML = '';
        }

        if (errors && errors.available && errors.errors.length > 0) {
            const rows = errors.errors.map(e =>
                `<tr><td>${e.timestamp}</td><td>${e.tool}</td><td>${e.outcome}</td><td>${e.message || '—'}</td></tr>`
            ).join('');
            document.getElementById('errors-list').innerHTML =
                `<table><thead><tr><th>Time</th><th>Tool</th><th>Type</th><th>Message</th></tr></thead><tbody>${rows}</tbody></table>`;
        } else {
            document.getElementById('errors-list').innerHTML = '<p class="muted">No recent errors.</p>';
        }
    }

    document.getElementById('stats-window').addEventListener('change', refreshStats);

    // --- Memories page ---
    let currentTopic = null;

    async function refreshMemories() {
        const data = await fetchJson('/api/memories');
        const topics = (data && data.topics) || [];
        const list = document.getElementById('memory-topics');
        list.innerHTML = topics.map(t =>
            `<li class="${t === currentTopic ? 'active' : ''}" data-topic="${t}">${t}</li>`
        ).join('');

        list.querySelectorAll('li').forEach(li => {
            li.addEventListener('click', () => loadMemory(li.dataset.topic));
        });
    }

    async function loadMemory(topic) {
        currentTopic = topic;
        const data = await fetchJson(`/api/memories/${encodeURIComponent(topic)}`);
        if (data && data.content !== undefined) {
            document.getElementById('memory-viewer').innerHTML =
                `<h3>${topic}</h3><pre>${escapeHtml(data.content)}</pre>` +
                `<div style="margin-top:1rem"><button class="btn btn-danger" onclick="deleteMemory('${topic}')">Delete</button></div>`;
        }
        refreshMemories();
    }

    window.deleteMemory = async function(topic) {
        if (!confirm(`Delete memory "${topic}"?`)) return;
        await fetch(`/api/memories/${encodeURIComponent(topic)}`, { method: 'DELETE' });
        currentTopic = null;
        document.getElementById('memory-viewer').innerHTML = '<p class="muted">Deleted.</p>';
        refreshMemories();
    };

    function escapeHtml(str) {
        return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    // --- Polling ---
    async function refreshAll() {
        await Promise.all([refreshOverview(), refreshStats(), refreshMemories()]);
        document.getElementById('last-refresh').textContent =
            'Last refreshed: ' + new Date().toLocaleTimeString();
    }

    refreshAll();
    setInterval(refreshAll, POLL_INTERVAL);
})();
```

**Step 2: Verify it compiles**

```bash
cargo check
```
Expected: clean compile.

**Step 3: Smoke test manually**

```bash
cargo run -- dashboard --project .
```
Expected: browser opens, dashboard loads with Overview page showing project info. Tool Stats and Memories pages should work (may show "not available" messages if no usage.db exists yet).

**Step 4: Commit**

```bash
cargo fmt
git add src/dashboard/static/
git commit -m "feat(dashboard): add frontend JavaScript — polling, charts, theme toggle"
```

---

### Task 9: Add `Serialize` derives where missing

**Files:**
- Modify: `src/config/project.rs` (ensure `ProjectConfig` and children derive `Serialize`)
- Modify: `src/embed/index.rs` (ensure `IndexStats`, `Staleness`, `DriftReportRow` derive `Serialize`)

**Context:** The dashboard serializes these structs to JSON via `serde_json::to_value()`. Some structs may already derive `Serialize` (check first). If not, add it.

**Step 1: Check existing derives**

Look at `ProjectConfig`, `ProjectSection`, `EmbeddingsSection`, `IgnoredPathsSection`, `SecuritySection` in `src/config/project.rs`. They likely derive `Deserialize` but may not derive `Serialize`. Also check `IndexStats`, `Staleness`, `DriftReportRow` in `src/embed/index.rs`.

**Step 2: Add missing `Serialize` derives**

For each struct that's missing `Serialize`, add it to the `#[derive(...)]` attribute. For example:

```rust
// Before:
#[derive(Debug, Deserialize)]
pub struct ProjectConfig { ... }

// After:
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct ProjectConfig { ... }
```

Do this for all structs that the dashboard API endpoints serialize.

**Step 3: Verify**

```bash
cargo check && cargo test
```
Expected: no regressions.

**Step 4: Commit**

```bash
cargo fmt
git add src/config/project.rs src/embed/index.rs
git commit -m "fix: add Serialize derives for dashboard JSON serialization"
```

---

### Task 10: Full test suite + clippy + manual smoke test

**Files:** None (verification only)

**Step 1: Run the full test suite**

```bash
cargo test
```
Expected: all tests pass (existing ~435 + new dashboard tests).

**Step 2: Clippy**

```bash
cargo clippy -- -D warnings
```
Expected: clean.

**Step 3: Format**

```bash
cargo fmt
```

**Step 4: Manual smoke test**

```bash
cargo run -- dashboard --project .
```

Verify in browser:
- Overview page: project name, root, languages, git branch, config, index status
- Tool Stats page: shows usage data (or "not available" message), window selector works
- Memories page: lists topics, clicking shows content
- Theme toggle works
- Auto-refresh updates "Last refreshed" every 5 seconds
- Ctrl+C in terminal shuts down cleanly

**Step 5: Final commit (if any formatting/clippy fixes needed)**

```bash
git add -u && git commit -m "chore: final cleanup — clippy + fmt"
```

---

## Summary

| Task | New files | Modified files | Phase |
|------|-----------|---------------|-------|
| 1 — Dependencies | — | `Cargo.toml` | — |
| 2 — Scaffold + CLI | `src/dashboard/mod.rs`, `routes.rs` | `src/lib.rs`, `src/main.rs`, `src/server.rs` | — |
| 3 — /api/project | `api/mod.rs`, `api/project.rs` | `routes.rs`, `mod.rs` | 1 |
| 4 — /api/config, index, drift | `api/config.rs`, `api/index.rs` | `api/mod.rs`, `routes.rs` | 1 |
| 5 — /api/usage, errors | `api/usage.rs`, `api/errors.rs` | `usage/db.rs`, `api/mod.rs`, `routes.rs` | 2 |
| 6 — /api/memories, libraries | `api/memories.rs`, `api/libraries.rs` | `api/mod.rs`, `routes.rs` | 3 |
| 7 — HTML + CSS | `static/index.html`, `static/dashboard.css` | `routes.rs` | — |
| 8 — JavaScript | `static/dashboard.js` | — | — |
| 9 — Serialize derives | — | `config/project.rs`, `embed/index.rs` | — |
| 10 — Verification | — | — | — |

Total: ~12 new files, ~8 modified, ~10 commits.

---

*Created: 2026-02-28*
*Design doc: `docs/plans/2026-02-28-project-dashboard-design.md`*
