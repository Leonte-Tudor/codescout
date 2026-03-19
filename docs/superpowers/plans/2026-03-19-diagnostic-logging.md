# Diagnostic Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `--diagnostic` flag that writes INFO-level lifecycle and tool-call-boundary events to a per-instance log file, enabling post-mortem analysis of MCP server disconnects.

**Architecture:** A new `--diagnostic` flag in clap + early arg peek triggers an INFO-level file layer in `logging::init()`. Each server instance generates a 4-hex-char random ID for its log filename and returns it so `run()` can enter a root span stamping every log line. Tool call boundaries get new `info!` events (existing `debug!` events unchanged). The `service.waiting()` result is destructured to log the exit reason.

**Tech Stack:** Rust, tracing, tracing-subscriber, tracing-appender (all existing deps)

**Spec:** `docs/superpowers/specs/2026-03-19-diagnostic-logging-design.md`

---

### Task 1: CLI flag + logging infrastructure

This task merges the CLI flag addition and logging changes into one compiling unit.

**Files:**
- Modify: `src/main.rs:14-39` (Commands::Start — add field)
- Modify: `src/main.rs:74-120` (main fn — early peek, new return type)
- Modify: `src/logging.rs:1-137` (rewrite init, add helpers, add tests)
- Modify: `src/server.rs:340-347` (run fn signature — add param)

- [ ] **Step 1: Write tests for `rotate_diagnostic_logs` and `generate_instance_id`**

Add to the `tests` module in `src/logging.rs`:

```rust
    #[test]
    fn rotate_diagnostic_keeps_last_6() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files with staggered mtimes
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }

        super::rotate_diagnostic_logs(dir.path());

        let mut remaining: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        remaining.sort();
        assert_eq!(remaining.len(), 6, "should keep exactly 6 files: {remaining:?}");
        // The two oldest (0000 and 0001) should be deleted
        assert!(!remaining.contains(&"diagnostic-0000.log".to_string()));
        assert!(!remaining.contains(&"diagnostic-0001.log".to_string()));
    }

    #[test]
    fn rotate_diagnostic_ignores_non_diagnostic_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files + 3 non-diagnostic files
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }
        std::fs::write(dir.path().join("debug.log"), "debug").unwrap();
        std::fs::write(dir.path().join("debug.log.1"), "debug old").unwrap();
        std::fs::write(dir.path().join("random.txt"), "other").unwrap();

        super::rotate_diagnostic_logs(dir.path());

        let all: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // 6 diagnostic + 3 non-diagnostic = 9
        assert_eq!(all.len(), 9, "non-diagnostic files must be untouched: {all:?}");
    }

    #[test]
    fn generate_instance_id_is_4_hex_chars() {
        let id = super::generate_instance_id();
        assert_eq!(id.len(), 4, "instance ID must be 4 chars: got '{id}'");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "instance ID must be hex: got '{id}'"
        );
    }

    #[test]
    fn generate_instance_id_varies_across_calls() {
        // RandomState is randomly seeded, so two calls should differ.
        // There's a 1/65536 chance of collision — acceptable for a test.
        let a = super::generate_instance_id();
        let b = super::generate_instance_id();
        assert_ne!(a, b, "instance IDs should vary across calls");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout rotate_diagnostic generate_instance_id -- --nocapture 2>&1 | tail -10`
Expected: FAIL — functions don't exist yet

- [ ] **Step 3: Implement `generate_instance_id` and `rotate_diagnostic_logs`**

Add to `src/logging.rs` (before `init`):

```rust
/// Generate a 4-hex-char random instance ID for log file naming.
/// Uses std::hash::RandomState which is randomly seeded per process.
fn generate_instance_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_usize(std::process::id() as usize);
    format!("{:04x}", hasher.finish() as u16)
}

/// Rotate diagnostic log files: keep the 6 most recent by mtime.
/// Different from `rotate_logs` which uses numbered backups for a single file.
pub fn rotate_diagnostic_logs(dir: &Path) {
    const KEEP: usize = 6;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("diagnostic-") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), mtime))
        })
        .collect();

    if entries.len() <= KEEP {
        return;
    }

    // Sort newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    // Remove everything beyond the 6th
    for (path, _) in &entries[KEEP..] {
        let _ = std::fs::remove_file(path);
    }
}
```

- [ ] **Step 4: Rewrite `init()` with new signature**

Replace the entire `init` function in `src/logging.rs` with:

```rust
/// Logging init result — holds worker guards and optional instance ID.
pub struct LoggingGuards {
    /// MUST be held for the lifetime of main. Dropping flushes and closes writers.
    pub guards: Vec<WorkerGuard>,
    /// 4-hex-char instance ID when diagnostic mode is active, for span injection.
    pub instance_id: Option<String>,
}

/// Initialise tracing.
///
/// - `debug`: enables DEBUG-level file logging to `.codescout/debug.log` (verbose)
/// - `diagnostic`: enables INFO-level file logging to `.codescout/diagnostic-<hash>.log`
///
/// Returns guards that MUST be held for the lifetime of `main`, plus the
/// diagnostic instance ID (if active) for root span injection.
pub fn init(debug: bool, diagnostic: bool) -> LoggingGuards {
    let mut guards = Vec::new();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));

    let log_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".codescout");

    if debug || diagnostic {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("codescout: could not create log directory: {e}");
        }
    }

    // --- Debug file layer (DEBUG level) ---
    let debug_layer = if debug {
        rotate_logs(&log_dir);
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join("debug.log"))
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("debug")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open debug log: {e}");
                None
            }
        }
    } else {
        None
    };

    // --- Diagnostic file layer (INFO level) ---
    let mut instance_id = None;
    let diagnostic_layer = if diagnostic {
        rotate_diagnostic_logs(&log_dir);
        let id = generate_instance_id();
        let filename = format!("diagnostic-{id}.log");
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join(&filename))
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                instance_id = Some(id);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("info")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open diagnostic log {filename}: {e}");
                None
            }
        }
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(debug_layer)
        .with(diagnostic_layer)
        .try_init()
        .ok();

    LoggingGuards {
        guards,
        instance_id,
    }
}
```

- [ ] **Step 5: Add `--diagnostic` to CLI and thread through**

In `src/main.rs`, inside `Commands::Start` (after `debug: bool` at line 38), add:

```rust
        /// Enable diagnostic logging to .codescout/diagnostic-<hash>.log
        #[arg(long)]
        diagnostic: bool,
```

Update `main()`:

```rust
    let debug_mode = std::env::args().any(|a| a == "--debug");
    let diagnostic_mode = std::env::args().any(|a| a == "--diagnostic");
    let _log_guards = codescout::logging::init(debug_mode, diagnostic_mode);
```

Update the `Commands::Start` match arm to destructure `diagnostic` and pass it:

```rust
            codescout::server::run(project, &transport, &host, port, auth_token, debug, diagnostic).await?;
```

- [ ] **Step 6: Add `diagnostic` parameter to `run()` in `src/server.rs`**

Change the signature (line 340) to add `diagnostic: bool` after `debug: bool`.
(No other changes in this step — the param is just threaded through for Tasks 2-3.)

- [ ] **Step 7: Run all tests + compile**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20
```

Expected: all pass, no warnings

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/server.rs src/logging.rs
git commit -m "feat(diagnostic): --diagnostic flag, INFO file layer with instance hash, rotation"
```

---

### Task 2: Log startup, service exit, heartbeat promotion

**Files:**
- Modify: `src/server.rs:340-422` (run fn — startup span, heartbeat promotion, exit reason)

- [ ] **Step 1: Enter root span with instance ID and log startup event**

The instance ID was generated in `logging::init()` but `run()` doesn't have access to it. We need to thread it through. Two options:

**Option chosen:** Read the instance ID from the log filename on disk. Simpler: just regenerate it. But the spec wants the SAME ID that's in the filename. So we pass it as a parameter.

Add `instance_id: Option<String>` to `run()` signature (or better: have `main()` pass it).

In `src/main.rs`, change the init + run calls:

```rust
    let log_state = codescout::logging::init(debug_mode, diagnostic_mode);
    // ... later in the Start match arm:
    codescout::server::run(project, &transport, &host, port, auth_token, debug, diagnostic, log_state.instance_id.clone()).await?;
```

Update `run()` signature in `src/server.rs`:

```rust
pub async fn run(
    project: Option<PathBuf>,
    transport: &str,
    host: &str,
    port: u16,
    auth_token: Option<String>,
    debug: bool,
    diagnostic: bool,
    instance_id: Option<String>,
) -> Result<()> {
```

Then after `let agent = Agent::new(project).await?;` (around line 354), add:

```rust
    // Enter a root span that stamps every subsequent log line with the instance ID.
    let instance_tag = instance_id.as_deref().unwrap_or("----");
    let _instance_span = tracing::info_span!("codescout", instance = %instance_tag).entered();

    if diagnostic {
        let project_display = agent
            .project_root()
            .await
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string());
        tracing::info!(
            pid = std::process::id(),
            version = env!("CARGO_PKG_VERSION"),
            instance = %instance_tag,
            project = %project_display,
            transport = %transport,
            "codescout_start"
        );
    }
```

- [ ] **Step 2: Promote heartbeat from `debug!` to `info!` and enable for diagnostic**

Change the heartbeat guard (line 357) from:
```rust
    if debug {
```
to:
```rust
    if debug || diagnostic {
```

Change the heartbeat log call (line 373) from:
```rust
                tracing::debug!(uptime_secs, active_projects, ?lsp_servers, "heartbeat");
```
to:
```rust
                tracing::info!(uptime_secs, active_projects, ?lsp_servers, "heartbeat");
```

(It only fires every 30s — negligible overhead even in the debug log.)

- [ ] **Step 3: Destructure `service.waiting()` to log exit reason**

Replace the `tokio::select!` block (around line 391-399):

From:
```rust
            tokio::select! {
                result = service.waiting() => {
                    result.map_err(|e| anyhow::anyhow!("MCP server exited: {}", e))?;
                }
                _ = shutdown_signal() => {
                    tracing::info!("Received shutdown signal");
                }
            }
```

To:
```rust
            tokio::select! {
                result = service.waiting() => {
                    match result {
                        Ok(reason) => tracing::info!(?reason, "service_exit"),
                        Err(e) => {
                            tracing::info!(%e, "service_exit join_error");
                            return Err(anyhow::anyhow!("MCP server exited: {}", e));
                        }
                    }
                }
                _ = shutdown_signal() => {
                    tracing::info!("service_exit reason=Signal");
                }
            }
```

Note: `reason` is `rmcp::service::QuitReason` which derives `Debug`. No import needed — the `?reason` format uses `Debug::fmt` and Rust infers the type from the `Ok` variant.

- [ ] **Step 4: Verify it compiles and tests pass**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20
```

Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/server.rs src/main.rs
git commit -m "feat(diagnostic): startup span, heartbeat promotion, service exit reason logging"
```

---

### Task 3: Tool call boundary INFO events

**Files:**
- Modify: `src/server.rs:126-215` (call_tool_inner — add info events alongside existing debug events)

- [ ] **Step 1: Add tool call start INFO event**

In `call_tool_inner`, after the existing `tracing::debug!(args = ?req.arguments, "tool call");` (line 132), add:

```rust
        let arg_keys: Vec<&str> = req
            .arguments
            .as_ref()
            .map(|m| m.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        tracing::info!(tool = %req.name, ?arg_keys, "tool_call");
        let tool_start = std::time::Instant::now();
```

- [ ] **Step 2: Add tool call end INFO event after the existing debug event**

Find the existing block (around lines 195-198):

```rust
        tracing::debug!(
            ok = call_result.is_error.map_or(true, |e| !e),
            "tool result"
        );
```

Add AFTER it (do not modify the existing `debug!` call):

```rust
        let ok = call_result.is_error.map_or(true, |e| !e);
        tracing::info!(
            tool = %req.name,
            duration_ms = tool_start.elapsed().as_millis() as u64,
            ok,
            "tool_done"
        );
```

Also update the existing `debug!` to reuse the `ok` variable:

```rust
        let ok = call_result.is_error.map_or(true, |e| !e);
        tracing::debug!(ok, "tool result");
        tracing::info!(
            tool = %req.name,
            duration_ms = tool_start.elapsed().as_millis() as u64,
            ok,
            "tool_done"
        );
```

- [ ] **Step 3: Verify it compiles and tests pass**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20
```

Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "feat(diagnostic): tool_call/tool_done INFO events with duration"
```

---

### Task 4: Full validation, release build, config updates

**Files:**
- Modify: `~/.claude/.claude.json` (add --diagnostic to args)
- Modify: `~/.claude-sdd/settings.json` (add --diagnostic to mcpServers.codescout.args)

- [ ] **Step 1: Run full test suite + lint**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Expected: all pass, no warnings

- [ ] **Step 2: Build release binary**

```bash
cargo build --release
```

Expected: compiles

- [ ] **Step 3: Smoke test — verify diagnostic log file is created with expected content**

```bash
timeout 3 target/release/codescout start --diagnostic 2>/dev/null || true
ls -la .codescout/diagnostic-*.log
cat .codescout/diagnostic-*.log
```

Expected: a `diagnostic-<hash>.log` file exists containing:
- A `codescout_start` line with pid, version, project, transport
- A `service_exit reason=Closed` line (stdin closes when timeout kills the process)

- [ ] **Step 4: Update `~/.claude/.claude.json`**

In the `mcpServers.codescout.args` array, change:
```json
"args": ["start"]
```
to:
```json
"args": ["start", "--diagnostic"]
```

- [ ] **Step 5: Update `~/.claude-sdd/settings.json`**

In the `mcpServers.codescout.args` array, change:
```json
"args": ["start"]
```
to:
```json
"args": ["start", "--diagnostic"]
```

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/server.rs src/logging.rs
git commit -m "feat(diagnostic): final validation pass"
```

(Only commit if there were formatting/lint fixes from Step 1. Skip if nothing changed.)

- [ ] **Step 7: Restart MCP servers**

Run `/mcp` in each Claude Code session to pick up the new `--diagnostic` flag.
