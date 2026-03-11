# Debug Logging Design

**Date:** 2026-03-11  
**Status:** Approved  
**Branch:** experiments

## Problem

codescout has sparse, inconsistent observability. When something goes wrong — a tool returns
unexpected output, an LSP server hangs, the server stops responding — there is no structured
way to diagnose it without recompiling with extra print statements. `RUST_LOG=debug` exists
but coverage is thin and the output only goes to stderr (which Claude Code swallows).

## Goals

- Persistent, file-backed debug log written to `.codescout/debug.log`
- Opt-in via `--debug` flag on `start` subcommand
- Automatic log rotation at startup (keep last 3 files)
- Full tool call tracing: name, args, duration, success/error
- LSP request/response timing and lifecycle events
- Heartbeat task to distinguish idle from hung server
- Zero overhead when not in debug mode

## Non-Goals

- Structured JSON log format (human-readable is sufficient)
- Log rotation mid-session (rotate only at startup)
- Tracing for `index` or `dashboard` subcommands (out of scope)
- Sampling or rate-limiting (debug mode is for development, not production)

## Approach: `#[tracing::instrument]` + file layer

Use `tracing-appender` to add a second subscriber layer that writes `debug`-and-above to a
file. Decorate key hot-path functions with `#[tracing::instrument]` to get structured spans
with timing and arguments automatically. Keep stderr at `info` level so it stays readable.

## Architecture

### 1. CLI Change

Add `--debug` flag to `Commands::Start` in `src/main.rs`:

```rust
/// Enable debug logging to .codescout/debug.log
#[arg(long)]
debug: bool,
```

The flag is forwarded to `logging::init(debug)` called at the top of `main` before the
`match cli.command` block. At that point the project path has not yet been destructured,
so logging always uses CWD as the base directory. `debug: bool` is also added to
`server::run()`'s signature so the heartbeat task can be conditionally spawned.

### 2. Log Initialization (`src/logging.rs`)

New module `src/logging.rs` owns all logging setup:

```rust
pub fn init(debug: bool) -> Option<tracing_appender::non_blocking::WorkerGuard>
```

Returns a `WorkerGuard` that must be held for the lifetime of `main` (dropping it flushes
the non-blocking writer). Returns `None` when not in debug mode.

When `debug = true`:
1. **Rotate** existing log files before opening:
   - `debug.log.3` → deleted
   - `debug.log.2` → `debug.log.3`
   - `debug.log.1` → `debug.log.2`
   - `debug.log`   → `debug.log.1`
2. **Open** `.codescout/debug.log` via `std::fs::OpenOptions`
3. **Wrap** with `tracing_appender::non_blocking()` (background writer thread)
4. **Build layered subscriber:**
   - Layer 1: stderr, level = `info`, format = compact human-readable
   - Layer 2: file, level = `debug`, format = full with timestamps + spans

Log format (file):
```
2026-03-11T14:23:01.452Z DEBUG codescout::tools::symbol  find_symbol{pattern="Foo" path=Some("src/")} — 23ms
```

The log directory is `.codescout/` relative to CWD (consistent with how other codescout
state is stored when no `--project` is given yet).

### 3. Instrumentation Sites

#### Tool dispatch — `src/server.rs::call_tool_inner`

Single highest-value site. `call_tool` is a thin shim; all real work (tool lookup, security
check, timeout, error routing) lives in `call_tool_inner`. Instrument that function:

```rust
#[tracing::instrument(skip_all, fields(tool = %req.name))]
async fn call_tool_inner(&self, req: CallToolRequestParam, progress: ...) -> Result<CallToolResult> { ... }
```

`skip_all` suppresses auto-logging of all parameters (avoids formatting large `Value` args
via `Debug`). The `fields` clause can still access `req.name` because it is evaluated in
the function's scope at span creation. Tool args are logged explicitly inside the span:
`tracing::debug!(args = %req.arguments)`. Captures: tool name, duration, ok/error.

#### LSP requests — `src/lsp/client.rs`

Add a span around `request_with_timeout` (the innermost JSON-RPC send site):

```rust
#[tracing::instrument(skip(self, params), fields(method = %method))]
async fn request_with_timeout(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> { ... }
```

Captures: LSP method name, duration, error. Response body logged at `debug!` level inside.
LSP stderr already routed via `tracing::debug!(target: "lsp_stderr", ...)` — keep as-is.

#### LSP lifecycle — `src/lsp/client.rs::LspClient::start`

Spawn and exit events are owned by `LspClient::start`, not `manager.rs`. Add `debug!` at:
- Language server spawn: binary path, PID
- Unexpected server exit: exit code, last lines of stderr

In `src/lsp/manager.rs`, add `debug!` only for events it owns:
- Shutdown initiated / completed per language

#### Server lifecycle — `src/server.rs`

Already has `info!` markers. Add `debug!` for:
- Signal received (which signal)
- Each LSP shutdown step

### 4. Heartbeat Task

Spawned inside `server::run()` only when `debug = true`. `server::run()` gains a `debug: bool`
parameter (threaded from `main`). Uses `tokio::time::interval(30s)`.
Requires a reference to `Arc<Agent>` (already available in `run()`).

```
DEBUG codescout  heartbeat{uptime_secs=30 active_projects=1 lsp_servers=["rust-analyzer","pyright"]}
```

Distinguishes "server alive but idle" from "server hung". Logs:
- `uptime_secs` — seconds since server start
- `active_projects` — number of activated projects (0 or 1 in practice)
- `lsp_servers` — list of running language server names

Task is `tokio::spawn`ed and its `JoinHandle` stored in a local; dropped cleanly on shutdown.

### 5. `skip` Annotations

Types requiring `skip` in `#[tracing::instrument]`:
- `ToolContext` — not `Debug`, contains `Arc` fields
- `serde_json::Value` for large tool args — skip, log manually with truncation
- `Arc<Agent>` — skip, log derived fields explicitly via `fields(...)`

### 6. New Dependency

```toml
# [dependencies]
tracing-appender = "0.2"

# [dev-dependencies]
tracing-test = "0.2"
```

`tracing-appender` is maintained by the `tokio-rs` org, same release cadence as
`tracing-subscriber`. The `non_blocking` wrapper is the only feature used.
`tracing-test` provides `#[traced_test]` for asserting log output in unit tests — used
only in the heartbeat and rotation tests.

## File Layout Changes

```
src/
├── logging.rs        # NEW — init(), rotate_logs(), WorkerGuard return
├── main.rs           # --debug flag, call logging::init(), hold guard; pass debug to server::run()
├── server.rs         # instrument call_tool_inner; debug: bool param; heartbeat task
├── lsp/
│   ├── client.rs     # instrument request_with_timeout; debug! on spawn/exit in LspClient::start
│   └── manager.rs    # debug! on shutdown lifecycle events
```

## Testing

- `logging::rotate_logs()` unit-tested with a `tempdir`: populate 4 files, rotate, assert names
- Heartbeat: unit test that the task emits at least one log line (use `tracing-test` crate)
- Instrument coverage: existing integration tests run with `RUST_LOG=debug` in CI to catch
  panics from bad `skip` annotations or non-Debug types

## Error Handling

- If `.codescout/` directory doesn't exist when debug mode starts: create it (same as
  how the memory store handles missing dirs)
- If log file can't be opened (permissions): print a warning to stderr and continue without
  the file layer — debug mode degrades gracefully, server still starts

## Out of Scope / Future

- Log viewer in the dashboard UI
- Per-tool log levels (e.g. `RUST_LOG=codescout::tools::symbol=trace`)
- Structured JSON format (can be added as a `--log-format` flag later)
- Mid-session rotation based on file size
