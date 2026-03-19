# Diagnostic Logging for Disconnect Debugging

**Date:** 2026-03-19
**Status:** Approved
**Problem:** codescout MCP server sometimes purely disconnects. No logs exist to determine why — the default INFO stderr output is lost when the process exits, and `--debug` is too verbose for always-on use.

## Design

### New `--diagnostic` CLI Flag

Add `--diagnostic` to the `start` subcommand. When enabled:

1. Generate a 4-hex-char random instance ID at startup using `getrandom::getrandom` (already transitively available) or `std::hash::RandomState`. Do not add a new dependency.
2. Create `.codescout/diagnostic-<hash>.log` with an INFO-level file layer
3. Rotate diagnostic logs (see Rotation Strategy below)
4. Enable the 30-second heartbeat (currently only behind `--debug`)
5. Coexists with `--debug` — they write to different files at different levels

The instance ID is injected as a tracing span field so every log line is attributable to a specific server instance.

### `--diagnostic` and `--debug` Interaction

Both flags can be active simultaneously. Each creates its own file layer with its own `WorkerGuard`. The return type of `logging::init()` changes from `Option<WorkerGuard>` to `Vec<WorkerGuard>` (or a small struct). **Both guards must be held in `main()` until process exit** — dropping a guard flushes and closes the non-blocking writer, which would lose late events like the `service_exit` line.

### Early-Peek in `main.rs`

The current `main.rs` peeks at raw `std::env::args()` for `--debug` before clap parses, because logging must initialize before anything else. The same early-peek pattern must be extended for `--diagnostic`: `args().any(|a| a == "--diagnostic")`. The `logging::init()` signature changes from `fn(debug: bool)` to `fn(debug: bool, diagnostic: bool)`.

### Rotation Strategy

The existing `rotate_logs()` uses a numbered-backup scheme for a single fixed filename (`debug.log` → `debug.log.1` → ...) and **cannot be reused** for diagnostic logs, which have per-instance filenames.

New rotation algorithm for diagnostic logs:
1. Glob `.codescout/diagnostic-*.log`
2. Sort by filesystem mtime descending
3. Remove all beyond the 6th most recent

Keep 6 files — covers ~3 per instance × 2 concurrent instances. The mtime race between concurrent instances starting simultaneously is benign (worst case: 7 files survive briefly).

### Disk Space Estimate

At INFO level with a 30-second heartbeat:
- ~400 bytes/min from heartbeats
- ~200 bytes per tool call (start + end lines)
- Heavy session (100 tool calls/hour, 8 hours): ~350KB per file
- 6-file cap: ~2MB max footprint — negligible

### Events Captured

**New INFO events added alongside existing DEBUG events** (the existing `debug!` calls remain unchanged — they carry richer data useful for `--debug` sessions):

- **Tool call start** (`info!`): tool name, argument keys only (not values)
- **Tool call end** (`info!`): tool name, duration ms, success/error status

The existing `debug!` events at lines 132 and 200-203 of `server.rs` log argument values and different fields. They stay as-is.

**New INFO events:**
- **Startup:** PID, instance hash, version, project root, transport type
- **Service exit reason:** `QuitReason` from `service.waiting()`. The current code discards the `Ok(reason)` variant via `map_err`. Must be changed to:
  ```rust
  result = service.waiting() => {
      match result {
          Ok(reason) => tracing::info!(?reason, "service_exit"),
          Err(e) => tracing::info!(%e, "service_exit join_error"),
      }
  }
  ```
  The `shutdown_signal()` branch must also log: `tracing::info!("service_exit reason=Signal")`.
- **Heartbeat:** uptime + active LSP servers every 30s. Distinguishes "hung" from "exited."

**Not promoted (stay at DEBUG):**
- Tool arguments and result bodies (too large)
- LSP message traffic
- Embedding pipeline details

### Example Log Output

```
2026-03-19T10:00:01Z INFO codescout[a3f1] pid=12345 version=0.5.0 project=/home/marius/work/claude/code-explorer transport=stdio
2026-03-19T10:00:01Z INFO codescout[a3f1] heartbeat uptime=0s lsp=[]
2026-03-19T10:00:02Z INFO codescout[a3f1] tool_call tool=activate_project
2026-03-19T10:00:02Z INFO codescout[a3f1] tool_done tool=activate_project duration=45ms ok=true
2026-03-19T10:00:31Z INFO codescout[a3f1] heartbeat uptime=30s lsp=["rust"]
...
2026-03-19T10:05:12Z INFO codescout[a3f1] service_exit reason=Closed
2026-03-19T10:05:12Z INFO codescout[a3f1] shutdown lsp_servers=["rust"]
```

### Files Changed

| File | Change |
|------|--------|
| `src/main.rs` | Add `--diagnostic` to clap + early-peek (`args().any(\|a\| a == "--diagnostic")`) |
| `src/logging.rs` | Signature → `fn(debug: bool, diagnostic: bool)` returning `Vec<WorkerGuard>`. New `rotate_diagnostic_logs()` helper (glob + mtime sort + keep 6). INFO file layer with instance hash. |
| `src/server.rs` → `run()` | Pass `diagnostic` to logging, enable heartbeat for diagnostic mode, log startup event, destructure `service.waiting()` result to log `QuitReason`, log shutdown signal branch |
| `src/server.rs` → `call_tool_inner()` | Add new `info!` events for tool call start/end (existing `debug!` lines unchanged) |

**Config (post-deploy, not CI-tested):**

| File | Change |
|------|--------|
| `~/.claude/.claude.json` | Add `"--diagnostic"` to codescout args |
| `~/.claude-sdd/settings.json` | Add `"--diagnostic"` to mcpServers.codescout.args |

### Not Changing

- No new dependencies
- No changes to `ToolContext`, `Tool` trait, or any tool implementation
- No changes to the rmcp integration beyond logging the exit reason
- No changes to the existing `--debug` path
