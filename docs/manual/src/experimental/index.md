# Experimental Features

Experimental features live on the [`experiments` branch](https://github.com/mareurs/codescout/tree/experiments/docs/manual/src/experimental).
Browse that branch to see what's in the works — each feature has its own page with full documentation.

When a feature graduates to stable, its page moves into the main manual here.

## Current experimental features

- [Elicitation-driven interactive sessions](./elicitation-interactive-sessions.md) — `run_command(interactive: true)` spawns a process with piped stdin/stdout and drives it via MCP elicitation in a loop. Suitable for setup wizards and slow-interaction CLIs.
- [PostCompact hook — LSP cache flush](./post-compact-cache-flush.md) — `project_status(post_compact: true)` evicts all LSP clients after context compaction; the companion plugin injects a directive to call it automatically.
- [Multi-ecosystem library auto-registration](./multi-ecosystem-auto-registration.md) — `activate_project` automatically detects and registers dependencies from Rust (Cargo), Node (npm), Python (pyproject/requirements), Go (go.mod), and Java/Kotlin (Gradle/Maven). Libraries without local source get a `source_available: false` flag and RecoverableError hints guiding agents to download sources.
- [activate_project output optimization](./activate-project-output-optimization.md) — `activate_project` now returns a slim orientation card (memories, index status, workspace siblings, RO/RW-conditional security fields) instead of the full raw config dump. Focus-switch by ID also returns the full card via the new `activate_within_workspace` helper.
