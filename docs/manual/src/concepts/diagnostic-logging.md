# Diagnostic Logging


The `--diagnostic` flag on the `start` command enables structured INFO-level
logging to a file in `.codescout/`. It is designed for debugging MCP disconnect
and silence issues without the noise of the full debug log.

## Enabling

Pass `--diagnostic` when starting the server:

```bash
codescout start --project /path/to/project --diagnostic
```

Or, if using `cargo run`:

```bash
cargo run -- start --project . --diagnostic
```

## Log file

Each server instance writes to its own file:

```
.codescout/diagnostic-<4hex>.log
```

The `<4hex>` suffix is a random 4-character hex instance ID unique to that
process. Old files are rotated automatically: the 6 most recent
`diagnostic-*.log` files are kept by modification time; older ones are deleted
at startup.

## What is logged

| Event | When | Fields |
|-------|------|--------|
| `codescout_start` | Server boot | `pid`, `version`, `project`, `transport`, `instance` |
| `heartbeat` | Every 30 s | `uptime_secs`, `active_projects`, `lsp_servers` |
| `tool_call` | Tool invoked | `tool`, `arg_keys` |
| `tool_done` | Tool returned | `tool`, `duration_ms`, `ok` |
| `service_exit` | Shutdown | `reason` (signal name or quit reason) |

All events are INFO level and written in a structured format alongside the
existing stderr INFO layer.

## Reading the log

```bash
cat .codescout/diagnostic-*.log
# or tail the most recent:
ls -t .codescout/diagnostic-*.log | head -1 | xargs tail -f
```

## When to use it

- **MCP client disconnects silently** — `service_exit` captures the shutdown
  reason (SIGHUP, SIGTERM, pipe close, etc.).
- **Tool call hangs** — compare `tool_call` and `tool_done` timestamps to find
  which tool never returned.
- **Heartbeat gaps** — a missing heartbeat indicates the server process was
  suspended or killed.

## Relationship to `--debug`

`--debug` enables verbose DEBUG-level file logging (`debug.log`) — useful for
LSP protocol tracing and detailed internal state. `--diagnostic` is a lighter
alternative: INFO only, one file per instance, focused on lifecycle and
tool-call boundaries.

Both flags can be used together.

## Limitations

- The instance ID (`<4hex>`) is derived from `RandomState` seeded at process
  start — it is not a cryptographic or globally unique ID.
- Log rotation is by mtime, not sequence number. If the filesystem does not
  update mtime reliably (some network filesystems), rotation order may be
  incorrect.
- `arg_keys` logs parameter names only, not values, to avoid capturing
  sensitive content in log files.
