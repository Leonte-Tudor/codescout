# Debug Logging Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in `--debug` mode to `codescout start` that writes structured debug logs to `.codescout/debug.log` with automatic rotation, tool call tracing, LSP instrumentation, and a heartbeat task.

**Architecture:** A new `src/logging.rs` module handles log file rotation and dual-layer subscriber setup (stderr=info, file=debug). Key hot-path functions (`call_tool_inner`, `request_with_timeout`) are decorated with `#[tracing::instrument]`. A heartbeat task (spawned only in debug mode) logs server liveness every 30 seconds.

**Tech Stack:** `tracing`, `tracing-subscriber` (already present), `tracing-appender = "0.2"` (new), `tracing-test = "0.2"` (dev, new)

**Spec:** `docs/superpowers/specs/2026-03-11-debug-logging-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `Cargo.toml` | Modify | Add `tracing-appender` dep, `tracing-test` dev-dep |
| `src/logging.rs` | Create | `rotate_logs()`, `init()`, `WorkerGuard` return |
| `src/main.rs` | Modify | `--debug` flag on `Start`, call `logging::init()`, pass `debug` to `server::run()` |
| `src/server.rs` | Modify | `call_tool_inner` instrumentation, `run()` gains `debug: bool`, heartbeat task |
| `src/lsp/client.rs` | Modify | `request_with_timeout` instrumentation, PID + exit debug logging in `start()` |
| `src/lsp/manager.rs` | Modify | `debug!` for shutdown success per language |

---

## Chunk 1: Logging module + CLI wiring

### Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add to `[dependencies]` and `[dev-dependencies]`**

In `Cargo.toml`, add under `# Logging / tracing`:
```toml
tracing-appender = "0.2"
```

And add a `[dev-dependencies]` section (or extend existing) with:
```toml
tracing-test = "0.2"
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```
Expected: no errors (may see "Compiling tracing-appender").

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add tracing-appender and tracing-test dependencies"
```

---

### Task 2: Create `src/logging.rs`

**Files:**
- Create: `src/logging.rs`

- [ ] **Step 1: Write the `rotate_logs` test first**

In the new `src/logging.rs`, write the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_keeps_last_3() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();

        // Populate 4 log files with their own name as content (for verification)
        for name in &["debug.log", "debug.log.1", "debug.log.2", "debug.log.3"] {
            std::fs::write(p.join(name), name.as_bytes()).unwrap();
        }

        rotate_logs(p);

        // Original debug.log.3 is deleted — no debug.log.4 should exist
        assert!(!p.join("debug.log.4").exists());
        // debug.log.3 now contains original debug.log.2 content
        assert_eq!(std::fs::read_to_string(p.join("debug.log.3")).unwrap(), "debug.log.2");
        // debug.log.2 now contains original debug.log.1 content
        assert_eq!(std::fs::read_to_string(p.join("debug.log.2")).unwrap(), "debug.log.1");
        // debug.log.1 now contains original debug.log content
        assert_eq!(std::fs::read_to_string(p.join("debug.log.1")).unwrap(), "debug.log");
        // debug.log itself is gone (renamed to .1)
        assert!(!p.join("debug.log").exists());
    }

    #[test]
    fn rotate_works_when_no_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        rotate_logs(dir.path()); // Must not panic
    }

    #[test]
    fn rotate_works_with_only_current_log() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("debug.log"), b"hello").unwrap();
        rotate_logs(p);
        assert!(!p.join("debug.log").exists());
        assert_eq!(std::fs::read_to_string(p.join("debug.log.1")).unwrap(), "hello");
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail (function not yet defined)**

```bash
cargo test rotate 2>&1 | tail -5
```
Expected: compile error — `rotate_logs` not found.

- [ ] **Step 3: Implement `src/logging.rs`**

```rust
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Rotate log files in `dir`: keep last 3 numbered backups.
/// debug.log.3 → deleted
/// debug.log.2 → debug.log.3
/// debug.log.1 → debug.log.2
/// debug.log   → debug.log.1
pub fn rotate_logs(dir: &Path) {
    const KEEP: u32 = 3;
    // Delete oldest
    let _ = std::fs::remove_file(dir.join(format!("debug.log.{}", KEEP)));
    // Shift numbered backups downward (highest first to avoid clobbering)
    for i in (1..KEEP).rev() {
        let from = dir.join(format!("debug.log.{}", i));
        let to = dir.join(format!("debug.log.{}", i + 1));
        let _ = std::fs::rename(from, to);
    }
    // Move current log to .1
    let _ = std::fs::rename(dir.join("debug.log"), dir.join("debug.log.1"));
}

/// Initialise tracing. When `debug` is true:
/// - Rotates `.codescout/debug.log` (keeps last 3)
/// - Adds a file layer at DEBUG level alongside the stderr INFO layer
/// - Returns a `WorkerGuard` that MUST be held for the lifetime of `main`
///   (dropping it flushes the non-blocking writer)
///
/// When `debug` is false, only the stderr INFO layer is installed.
/// Returns `None` in that case.
pub fn init(debug: bool) -> Option<WorkerGuard> {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        );

    if debug {
        let log_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".codescout");

        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("codescout: could not create log directory: {e}");
        }

        rotate_logs(&log_dir);

        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join("debug.log"))
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_filter(EnvFilter::new("debug"));

                tracing_subscriber::registry()
                    .with(stderr_layer)
                    .with(file_layer)
                    .init();

                return Some(guard);
            }
            Err(e) => {
                eprintln!("codescout: could not open debug log, falling back to stderr only: {e}");
            }
        }
    }

    tracing_subscriber::registry().with(stderr_layer).init();
    None
}
```

- [ ] **Step 4: Register the module in `src/lib.rs`**

`src/lib.rs` exists and owns all public modules. Add to `src/lib.rs`:
```rust
pub mod logging;
```

In `src/main.rs`, import it via the crate path (the binary links against the library):
```rust
use codescout::logging;
```

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cargo test rotate 2>&1 | tail -10
```
Expected: 3 tests pass.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep -v "^$"
```
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add src/logging.rs src/lib.rs  # or src/main.rs, whichever has the mod declaration
git commit -m "feat(logging): add rotate_logs and init() with dual-layer subscriber"
```

---

### Task 3: Wire `--debug` into `main.rs` and update `server::run()` signature

**Files:**
- Modify: `src/main.rs`
- Modify: `src/server.rs` (signature only)

- [ ] **Step 1: Add `--debug` to `Commands::Start` in `src/main.rs`**

In the `Start { ... }` enum variant, add after `auth_token`:
```rust
/// Enable debug logging to .codescout/debug.log
#[arg(long)]
debug: bool,
```

- [ ] **Step 2: Replace the existing tracing setup in `main()` with `logging::init()`**

Remove the current block:
```rust
tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    .init();
```

And the associated `use` statements for `EnvFilter`. Replace with a call to `codescout::logging::init()` placed at the **top of `main()`** before `let cli = Cli::parse()`:

```rust
// Logging init happens before CLI parsing so startup errors are captured.
// We peek at raw args to detect --debug before clap processes them.
// Caveat: this fires for any subcommand that receives "--debug" as an argument.
// Currently only `start` has --debug, so this is safe — revisit if other
// subcommands add conflicting flags.
let debug_mode = std::env::args().any(|a| a == "--debug");
let _log_guard = codescout::logging::init(debug_mode);
```

Also remove the now-unused import from `src/main.rs`:
```rust
// DELETE this line:
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
```

- [ ] **Step 3: Destructure `debug` in the `Commands::Start` match arm and pass to `server::run()`**

In the `Commands::Start { project, transport, host, port, auth_token }` match arm, add `debug`:
```rust
Commands::Start {
    project,
    transport,
    host,
    port,
    auth_token,
    debug,
} => {
    tracing::info!("Starting codescout MCP server (transport={})", transport);
    codescout::server::run(project, &transport, &host, port, auth_token, debug).await?;
}
```

- [ ] **Step 4: Update `server::run()` signature to accept `debug: bool`**

In `src/server.rs`, add `debug: bool` as the last parameter of `run()`:
```rust
pub async fn run(
    project: Option<PathBuf>,
    transport: &str,
    host: &str,
    port: u16,
    auth_token: Option<String>,
    debug: bool,
) -> Result<()> {
```

The body doesn't use `debug` yet (heartbeat comes in Task 5). Add `let _ = debug;` temporarily to silence the unused warning.

- [ ] **Step 5: Build and confirm no compile errors**

```bash
cargo build 2>&1 | head -20
```
Expected: clean build.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/server.rs
git commit -m "feat(logging): wire --debug flag through CLI and server::run()"
```

---

## Chunk 2: Tool instrumentation + heartbeat

### Task 4: Instrument `call_tool_inner`

**Files:**
- Modify: `src/server.rs`

`call_tool_inner` is the innermost dispatch point — every tool call flows through it. Adding `#[tracing::instrument]` here captures tool name, duration, and success/error for all 28 tools with one annotation.

- [ ] **Step 1: Add the instrument attribute to `call_tool_inner`**

Add this attribute directly above `async fn call_tool_inner`:
```rust
#[tracing::instrument(skip_all, fields(tool = %req.name))]
```

`skip_all` suppresses auto-Debug-formatting of all parameters (avoids dumping large `Value` args into the log). The `fields` clause accesses `req.name` which is in scope at span creation despite `skip_all`.

- [ ] **Step 2: Log tool arguments explicitly at the top of `call_tool_inner`**

Add this as the first line inside `call_tool_inner`, right after the opening brace:
```rust
tracing::debug!(args = ?req.arguments, "tool call");
```

This logs the full argument map. The `?` format uses `Debug`, which for `serde_json::Map` produces readable output.

- [ ] **Step 3: Log the result before returning**

Just before `Ok(strip_project_root_from_result(...))`, add:
```rust
tracing::debug!(
    ok = call_result.is_error.map_or(true, |e| !e),
    "tool result"
);
```

- [ ] **Step 4: Build**

```bash
cargo build 2>&1 | head -20
```
Expected: clean. If `CallToolResult` doesn't have `is_error`, use `matches!(call_result.content.first(), ...)` or simply log `"tool done"` without the ok field.

- [ ] **Step 5: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/server.rs
git commit -m "feat(logging): instrument call_tool_inner with tracing span"
```

---

### Task 5: Add heartbeat task

**Files:**
- Modify: `src/server.rs`

The heartbeat is a `tokio::spawn`ed loop that logs server liveness every 30 seconds. It only runs when `debug = true`. It uses `LspManager::active_languages()` (sync, already exists) and `Agent::project_root()` (async).

- [ ] **Step 1: Add the heartbeat spawner to `server::run()`**

In `server::run()`, after `let lsp = LspManager::new_arc();` and before the `match transport` block, add:

```rust
// Heartbeat: only in debug mode — distinguishes idle from hung.
if debug {
    let agent_hb = agent.clone();
    let lsp_hb = lsp.clone();
    let start = tokio::time::Instant::now();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.tick().await; // Skip the immediate first tick
        loop {
            interval.tick().await;
            let uptime_secs = start.elapsed().as_secs();
            let lsp_servers = lsp_hb.active_languages().await;
            let active_projects: usize =
                if agent_hb.project_root().await.is_some() { 1 } else { 0 };
            tracing::debug!(
                uptime_secs,
                active_projects,
                ?lsp_servers,
                "heartbeat"
            );
        }
    });
}
```

Remove the `let _ = debug;` placeholder added in Task 3.

- [ ] **Step 2: Verify `LspManager::active_languages()` is accessible**

`active_languages` is `pub async fn` on `impl LspManager` (it acquires a tokio async Mutex internally). Since `lsp` is `Arc<LspManager>`, call it as `lsp_hb.active_languages().await`. The heartbeat snippet above already uses `.await` — confirm it is present.

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | head -20
```
Expected: clean. Fix any borrow/lifetime issues (e.g. if `project_root()` returns a different type than `Option<_>`).

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -20
```
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs
git commit -m "feat(logging): add heartbeat task in debug mode"
```

---

## Chunk 3: LSP instrumentation

### Task 6: Instrument `request_with_timeout` and add PID/exit logging in `LspClient::start`

**Files:**
- Modify: `src/lsp/client.rs`

`request_with_timeout` is called for every LSP request (`textDocument/definition`, `textDocument/hover`, etc.). Instrumenting it gives timing for every LSP roundtrip. The `start()` function already logs the server command at `info!` — we add `debug!` for the PID and for unexpected exits.

- [ ] **Step 1: Add `#[tracing::instrument]` to `request_with_timeout`**

Add this attribute directly above `pub async fn request_with_timeout`:
```rust
#[tracing::instrument(skip(self, params, timeout), fields(lsp_method = %method))]
```

Skips `self` (not Debug), `params` (large Value), `timeout` (not interesting as a field). Captures `method` as `lsp_method` in the span — this appears in every line of the debug log during the request.

- [ ] **Step 2: Log the response at the end of `request_with_timeout`**

At the `Ok(Ok(result)) => result` arm of the outer `match`, change to:
```rust
Ok(Ok(result)) => {
    tracing::debug!(response_bytes = result.to_string().len(), "lsp response");
    result
}
```

This logs the size of the response without dumping potentially large bodies.

- [ ] **Step 3: Add PID debug logging in `LspClient::start`**

In `start()`, after `let child_pid = child.id();`, add:
```rust
tracing::debug!(
    pid = ?child_pid,
    binary = %config.command,
    "LSP server spawned"
);
```

- [ ] **Step 4: Add exit code logging in the reader task's Err branch**

In `start()`, inside the spawned reader task, in the `Err(e)` branch (the "EOF or read error — server crashed" block), after the `tracing::warn!("LSP reader error: {}", e)` line, add:
```rust
// Try to get the exit status for diagnostics.
// try_wait() returns Ok(None) if the child is still running (rare at EOF),
// Ok(Some(status)) if it has exited, or Err if the call itself failed.
match child.try_wait() {
    Ok(Some(status)) => tracing::debug!(exit_status = ?status, "LSP server exited"),
    Ok(None) => tracing::debug!("LSP reader EOF but child still running"),
    Err(e) => tracing::debug!("could not get LSP exit status: {e}"),
}
```

> **Note:** `child` is already moved into this spawned task (the existing `child.wait().await` at the end of the task uses it). `try_wait(&mut self)` requires a mutable reference — `child` is owned by the task, so this compiles.

- [ ] **Step 5: Build**

```bash
cargo build 2>&1 | head -20
```
Expected: clean.

- [ ] **Step 6: Run tests**

```bash
cargo test 2>&1 | tail -20
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/lsp/client.rs
git commit -m "feat(logging): instrument LSP request_with_timeout and add PID/exit debug logging"
```

---

### Task 7: Add debug! for shutdown success in `manager.rs`

**Files:**
- Modify: `src/lsp/manager.rs`

`shutdown_all` already has `info!` for each language being shut down and `warn!` on error. Adding `debug!` for the success case completes the per-language lifecycle trace.

- [ ] **Step 1: Add success debug log in `shutdown_all`**

Current code:
```rust
for (lang, client) in clients.drain() {
    tracing::info!("Shutting down LSP for: {}", lang);
    if let Err(e) = client.shutdown().await {
        tracing::warn!("Error shutting down LSP for {}: {}", lang, e);
    }
}
```

Change to:
```rust
for (lang, client) in clients.drain() {
    tracing::info!("Shutting down LSP for: {}", lang);
    match client.shutdown().await {
        Ok(()) => tracing::debug!("LSP server shut down cleanly: {}", lang),
        Err(e) => tracing::warn!("Error shutting down LSP for {}: {}", lang, e),
    }
}
```

- [ ] **Step 2: Build and test**

```bash
cargo build 2>&1 | head -5 && cargo test 2>&1 | tail -10
```
Expected: clean build, all tests pass.

- [ ] **Step 3: Run full quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```
Expected: clean.

- [ ] **Step 4: Final commit**

```bash
git add src/lsp/manager.rs
git commit -m "feat(logging): add debug! for LSP shutdown success in manager"
```

---

## Final Verification

- [ ] **Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

- [ ] **Smoke test `--debug` flag**

```bash
# In a terminal, start in debug mode:
./target/release/codescout start --project . --debug &
sleep 2
ls -la .codescout/debug.log
cat .codescout/debug.log | head -20
kill %1
```
Expected: `debug.log` exists, contains timestamped DEBUG lines.

- [ ] **Smoke test rotation**

Run again:
```bash
./target/release/codescout start --project . --debug &
sleep 1 && kill %1
ls .codescout/debug.log*
```
Expected: `debug.log` and `debug.log.1` both exist.

- [ ] **Restart MCP server**

```
/mcp
```
Expected: server restarts cleanly. No panics or errors.
