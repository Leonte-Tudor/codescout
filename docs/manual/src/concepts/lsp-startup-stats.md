# LSP Startup Statistics

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
