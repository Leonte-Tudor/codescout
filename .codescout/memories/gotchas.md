## Cross-Project Gotchas

### Active Project Switching Bug
Multiple onboarding agents reported that `activate_project` does not reliably persist between
consecutive MCP calls — the active project silently switches to a different fixture project.
Workaround: use explicit `project_id` parameters or full paths from the workspace root.
Tracked: needs investigation in `src/agent.rs` project focus logic.

### Fixture Path Resolution
When activated on a fixture project, `read_file` and `run_command` may resolve relative paths
against the workspace root (code-explorer) instead of the fixture root. Use full relative paths
from workspace root: `tests/fixtures/<lang>-library/src/...`

### find_symbol Cross-Project Leakage
`find_symbol` without a `path` constraint returns results from ALL workspace projects, not just
the activated one. Always scope with `path="tests/fixtures/<lang>-library"` or use `project_id`.

### Tree-Sitter Gaps
- Kotlin: top-level functions at end of file (after class definitions) may be missed by tree-sitter
- TypeScript: `find_symbol` can return "suspicious range" errors for globally-scoped searches

### Parallel Write Safety (BUG-021)
NEVER dispatch parallel write tool calls (`edit_file`, `replace_symbol`, `insert_code`, `create_file`).
See MEMORY.md "Parallel Write Safety" for full details.

### Kotlin LSP Multi-Instance
Kotlin LSP (kotlin-language-server) cannot run multiple instances on the same project.
The LSP mux (`src/lsp/mux/`) shares a single JVM instance across sessions via Unix sockets.
See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.