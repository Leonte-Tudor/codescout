# Experimental Features

Experimental features live on the [`experiments` branch](https://github.com/mareurs/codescout/tree/experiments/docs/manual/src/experimental).
Browse that branch to see what's in the works — each feature has its own page with full documentation.

When a feature graduates to stable, its page moves into the main manual here.

## Current experimental features

- [Elicitation-driven interactive sessions](./elicitation-interactive-sessions.md) — `run_command(interactive: true)` spawns a process with piped stdin/stdout and drives it via MCP elicitation in a loop. Suitable for setup wizards and slow-interaction CLIs.
- [PostCompact hook — LSP cache flush](./post-compact-cache-flush.md) — `project_status(post_compact: true)` evicts all LSP clients after context compaction; the companion plugin injects a directive to call it automatically.
- [Auto-register Cargo dependencies](./auto-register-cargo-deps.md) — `activate_project` on a Rust project automatically registers direct Cargo deps from `~/.cargo/registry` so symbol tools can search inside them without manual `register_library` calls.
