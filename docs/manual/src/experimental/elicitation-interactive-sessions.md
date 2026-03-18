# Elicitation-Driven Interactive Sessions

> ⚠ Experimental — may change without notice.

`run_command` now supports an `interactive: true` parameter that spawns the
process with piped stdin/stdout/stderr and drives it via MCP elicitation in a
loop. Instead of a `session_send`/`session_cancel` multi-tool protocol, the
entire session fits in one tool call.

## Usage

```json
{
  "command": "python3 -c \"name=input('Your name? '); print(f'Hello, {name}!')\"",
  "interactive": true
}
```

Each loop iteration:
1. Reads available output (with a 150 ms settle window to batch bursts).
2. Shows accumulated output in an elicitation dialog.
3. Waits for the user to type input (or leave empty to cancel).
4. Sends the input to the process stdin.
5. Repeats until the process exits naturally or the user cancels.

## Parameters

| Parameter | Type | Notes |
|-----------|------|-------|
| `command` | string | Shell command to run (required). |
| `interactive` | boolean | Set `true` to enable interactive mode. |
| `cwd` | string | Subdirectory relative to project root. |
| `timeout_secs` | integer | Ignored in interactive mode (reserved for future use). |

## Cancellation

Leave the input field empty in the elicitation dialog to cancel. The process is
killed with `SIGKILL` and accumulated output is returned.

A safety cap of 50 elicitation rounds is enforced. If reached, the process is
killed and a `[interactive: max rounds reached, process killed]` note is
appended to the output.

## Return value

```json
{
  "exit_code": 0,
  "stdout": "<accumulated stdout + stderr>",
  "interactive_rounds": 3
}
```

When the process is killed (user cancel, max rounds, or write error):

```json
{
  "exit_code": -1,
  "stdout": "<accumulated output>\n[interactive: cancelled by user]",
  "interactive_rounds": 2,
  "note": "process killed or loop exited before natural termination"
}
```

## Limitations

- **Latency**: each round-trip through MCP elicitation adds ~1–3 s. Suitable
  for setup wizards and slow-paced REPLs; not suitable for ncurses TUIs,
  editors, or programs expecting sub-second responses.
- **Settle heuristic**: the 150 ms silence window may split a logical prompt
  across two rounds if the program emits output in bursts with longer pauses.
- **Dangerous commands**: `interactive: true` blocks dangerous commands
  outright (no elicitation confirmation). Use the standard non-interactive path
  with `acknowledge_risk: true` if needed.
- **No elicitation fallback**: if the MCP client does not support elicitation,
  a `RecoverableError` is returned immediately. There is no non-interactive
  fallback — use `interactive: false` (the default) in that case.
- **No test coverage**: integration testing requires a live MCP peer with
  elicitation support.
