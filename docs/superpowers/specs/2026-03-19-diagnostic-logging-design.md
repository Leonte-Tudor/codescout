# Diagnostic Logging for Disconnect Debugging

**Date:** 2026-03-19
**Status:** Approved
**Problem:** codescout MCP server sometimes purely disconnects. No logs exist to determine why — the default INFO stderr output is lost when the process exits, and `--debug` is too verbose for always-on use.

## Design

### New `--diagnostic` CLI Flag

Add `--diagnostic` to the `start` subcommand. When enabled:

1. Generate a 4-hex-char random instance ID at startup (e.g., `a3f1`)
2. Create `.codescout/diagnostic-<hash>.log` with an INFO-level file layer
3. Rotate: keep the last 6 `diagnostic-*.log` files (covers ~3 per instance × 2 concurrent instances)
4. Enable the 30-second heartbeat (currently only behind `--debug`)
5. Coexists with `--debug` — they write to different files at different levels

The instance ID is injected as a tracing span field so every log line is attributable to a specific server instance.

### Events Captured

**Promoted from DEBUG to INFO** (compact one-liners, always useful):
- Tool call start: tool name, argument keys (not values)
- Tool call end: tool name, duration ms, success/error status

**New INFO events:**
- **Startup:** PID, instance hash, version, project root, transport type
- **Service exit reason:** `QuitReason` from `service.waiting()` — `Closed`, `Cancelled`, or `JoinError`. This is the single most diagnostic line for disconnect root-cause.
- **Heartbeat:** uptime + active LSP servers every 30s. Distinguishes "hung" from "exited" when reading logs after a disconnect.

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
| `src/main.rs` | Add `--diagnostic` flag to `start` subcommand |
| `src/logging.rs` | Accept `diagnostic: bool`, rotation for `diagnostic-*.log` (keep 6), INFO file layer with instance hash |
| `src/server.rs` → `run()` | Pass `diagnostic` to logging, enable heartbeat, log startup event + `QuitReason` + shutdown |
| `src/server.rs` → `call_tool_inner()` | Promote tool call start/end from `debug!` to `info!` |
| `~/.claude/.claude.json` | Add `"--diagnostic"` to codescout args |
| `~/.claude-sdd/settings.json` | Add `"--diagnostic"` to codescout mcpServers args |

### Not Changing

- No new dependencies
- No changes to `ToolContext`, `Tool` trait, or any tool implementation
- No changes to the rmcp integration beyond logging the exit reason
- No changes to the existing `--debug` path
