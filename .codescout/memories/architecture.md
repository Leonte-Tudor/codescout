# Architecture

See `docs/ARCHITECTURE.md` for the full component diagram. This memory captures
wiring details NOT covered there.

## Source Tree

```
src/
├── main.rs          # CLI: start (MCP server), index, and dashboard subcommands
├── lib.rs           # Crate root for library/integration use
├── server.rs        # rmcp ServerHandler — bridges Tool trait to MCP, signal handling + graceful LSP shutdown
├── agent.rs         # Orchestrator: active project, config, memory
├── workspace.rs     # Workspace/Project/DiscoveredProject — multi-project discovery and focus switching
├── logging.rs       # --debug mode: file logging with rotation (tracing-appender)
├── config/          # ProjectConfig (.codescout/project.toml), modes
├── lsp/             # LSP types, server configs (9 langs), JSON-RPC client
│   ├── mux/         #   Unix socket multiplexer — shares one LSP process across multiple clients
├── ast/             # Language detection (20+ exts), tree-sitter parser
├── git/             # git2: blame, file_log, open_repo
├── embed/           # Chunker, SQLite index, RemoteEmbedder, schema, drift detection
├── library/         # LibraryRegistry, Scope enum, manifest discovery, auto_register
├── memory/          # Markdown-based MemoryStore (.codescout/memories/)
├── usage/           # UsageRecorder: append-only SQLite call stats (usage.db)
├── prompts/         # LLM guidance: server_instructions.md, onboarding_prompt.md, workspace_onboarding_prompt.md
├── tools/           # Tool implementations by category
│   ├── output.rs          #   OutputGuard: progressive disclosure (exploring/focused)
│   ├── output_buffer.rs   #   OutputBuffer: session-scoped LRU (@cmd_*/@file_*/@tool_* handles)
│   ├── section_coverage.rs #  SectionCoverage: tracks which markdown headings have been read this session
│   ├── progress.rs        #   ProgressReporter: MCP progress notifications
│   ├── format.rs          #   Shared format helpers (format_line_range, format_overflow, truncate_path)
│   ├── file.rs            #   read_file, list_dir, grep, search_pattern, create_file, find_file, edit_file, glob
│   ├── markdown.rs        #   read_markdown, edit_markdown (heading-based section navigation + editing)
│   ├── file_summary.rs    #   Smart per-type summarizers (source, markdown, JSON, TOML, YAML)
│   ├── workflow.rs        #   onboarding, run_command
│   ├── symbol.rs          #   9 LSP-backed tools (find_symbol, list_symbols, goto_definition, hover, remove_symbol, etc.)
│   ├── git.rs             #   git_blame, file_log (not registered; used by dashboard)
│   ├── semantic.rs        #   semantic_search, index_project, index_status
│   ├── github.rs          #   github_identity, github_issue, github_pr, github_file, github_repo
│   ├── library.rs         #   list_libraries, register_library
│   ├── memory.rs          #   memory (action: read/write/list/delete/remember/recall/forget/refresh_anchors)
│   ├── usage.rs           #   GetUsageStats (dashboard API; not an MCP tool)
│   ├── ast.rs             #   list_functions, list_docs (not registered; tree-sitter offline tools)
│   ├── command_summary.rs #   Smart output summarization, terminal filter detection
│   └── config.rs          #   activate_project, project_status
└── util/            # fs helpers, text processing, path security
    └── path_security.rs   # SecurityProfile, check_tool_access, validate_write_path
```

## Registered Tools (27 total, as of v0.7.2)

File: ReadFile, ListDir, Grep, CreateFile, Glob, EditFile, EditMarkdown, ReadMarkdown
Workflow: RunCommand, Onboarding
Symbol (LSP): FindSymbol, FindReferences, GotoDefinition, Hover, ListSymbols, ReplaceSymbol, RemoveSymbol, InsertCode, RenameSymbol
Memory: Memory
Semantic: SemanticSearch, IndexProject, IndexStatus
Config: ActivateProject, ProjectStatus
Library: ListLibraries, RegisterLibrary

## Tool Dispatch Pipeline (concrete flow)

`rmcp::ServerHandler::call_tool` → `call_tool_inner` (src/server.rs):
1. `find_tool(name)` — linear scan over `Vec<Arc<dyn Tool>>`
2. `check_tool_access(name, &security)` — match-arm gate in `src/util/path_security.rs`
3. Build `ToolContext { agent, lsp, output_buffer, progress, peer, section_coverage }`
4. Apply `tool_timeout_secs` from `project.toml` (skipped for `index_project`, `onboarding`)
5. `UsageRecorder::record_content` wraps `tool.call_content()`
6. `route_tool_error`: `RecoverableError` → `isError:false` + JSON error/hint;
   LSP code -32800 → recoverable with Kotlin multi-session hint;
   other errors → `isError:true`
7. `strip_project_root_from_result` removes absolute project prefix from all output text

## Output Routing (Tool trait default impl)

`call_content()` in `src/tools/mod.rs`:
- Small output (< `MAX_INLINE_TOKENS`) → pretty-printed JSON inline
- Large output → stored in `OutputBuffer::store_tool()` as `@tool_xxx` ref (LRU, 50 slots)
  Returns `{ output_id, summary, hint }` where hint points to `json_path` or line range

`OutputGuard` (src/tools/output.rs) enforces two modes:
- `Exploring` (default): cap at 200 items / 200 files, no body inclusion
- `Focused` (`detail_level: "full"`): paginated via offset/limit, includes bodies

## LSP Lifecycle

`LspManager::get_or_start(language, root)` (src/lsp/manager.rs):
- Fast path: cache hit by LspKey (language + workspace_root), checks `is_alive()`
- Circuit breaker: after `CIRCUIT_BREAKER_MAX_FAILURES` in the window, returns error immediately
- Mux path (Unix only): if `config.mux == true`, routes to `get_or_start_via_mux()` — uses
  Unix socket to connect to a shared LSP process (avoids spawning per-client)
- LRU eviction: if at `max_clients` capacity, shuts down least-recently-used before starting new
- Slow path: watch-channel barrier deduplicates concurrent cold-starts for the same language key.
  First caller becomes "starter" (holds `tx`); others wait on `rx.wait_for(|v| v.is_some())`.
- `StartingCleanup` RAII guard removes barrier on any exit path (including async cancellation)

## Embedding Pipeline (build_index)

`build_index(root, force)` in `src/embed/index.rs`:
1. `find_changed_files()`: git diff → mtime → SHA-256 fallback
2. `ast_chunker::split_file()`: AST-aware chunking per language
3. Concurrent embedding: `JoinSet` over `Embedder::embed()`, semaphore cap=4
4. Single SQLite transaction: delete old chunks, insert new, upsert file hash
5. Drift detection: cosine distance old→new embeddings → `drift_report` table
6. High-drift files → mark memory anchors stale

**sqlite-vec**: Extension loading via `init_sqlite_vec()` and `maybe_migrate_to_vec0()` — vec0
virtual tables are active when available (KNN search). Falls back to pure-Rust cosine scan
loading all embeddings into memory. Check `is_vec0_active()` to determine current mode.

## Memory Architecture (two tiers)

- **File store**: Markdown in `.codescout/memories/`, CRUD via `MemoryStore`
- **Semantic store**: Vector embeddings in `.codescout/embeddings.db`.
  `remember`/`recall`/`forget` actions. Auto-classification via `classify_bucket()`.
- **Anchor sidecars**: `.anchors.toml` tracks source file SHA-256 for staleness detection.
  Regenerated on each `write`; cleared via `refresh_anchors` action.
- **Workspace memories**: Each sub-project gets its own memory dir via
  `Workspace::memory_dir_for_project()`

## Markdown Tools (new as of experiments branch)

`ReadMarkdown` / `EditMarkdown` in `src/tools/markdown.rs`:
- `read_markdown`: heading-based section navigation; records sections read in `SectionCoverage`
- `edit_markdown`: section-level replace/insert/remove/edit actions; scoped text replacement
  within a heading's body; batch multi-section edits
- `SectionCoverage` (src/tools/section_coverage.rs): session-scoped map of path → read headings.
  On read, emits `unread_hint` listing sections not yet seen. Invalidates on file mtime change.

## MCP Elicitation

`ToolContext::elicit(schema, title, message)` in `src/tools/mod.rs`:
- Uses `peer` field (`Peer<RoleServer>`) to send an MCP elicitation request to the client
- Pauses tool execution and waits for user response
- Only works in clients that support elicitation (checked at session start via `is_subagent_capable`)

## Library Auto-Registration

`auto_register_deps(project_root, ctx)` in `src/library/auto_register.rs`:
- Called on `activate_project`; parses Cargo.toml, package.json, pyproject.toml,
  requirements.txt, go.mod, build.gradle, pom.xml
- For each dep: checks if source is available locally (cargo registry cache, node_modules,
  venv, go mod cache) and calls `register_library` if so
- Returns `Vec<RegisteredDep>` included in activate_project response

## Unregistered Tool Structs

- `ListFunctions`, `ListDocs` (src/tools/ast.rs) — tree-sitter offline tools, used by dashboard
- `GetUsageStats` (src/tools/usage.rs) — dashboard API only
- GitHub tools are registered (github_identity etc.) but gated by `github_enabled` config

## Server Instructions

Pre-computed at construction in `from_parts` via `build_server_instructions()`.
For stdio: reflects state at startup, never refreshed mid-session.
For HTTP/SSE: each connection gets fresh instructions.
Custom instructions loaded from `.codescout/system-prompt.md` if present.

## Invariants

| Rule | Why it exists |
|---|---|
| Write tools must appear in `check_tool_access` match arm | Missing entry bypasses access gate silently |
| `RecoverableError` for expected failures, `bail!` for bugs | Controls whether Claude Code aborts sibling parallel calls |
| Write tools return `json!("ok")` | Echoing content wastes tokens with zero info gain |
| `call_content()` is the MCP entry point, NOT `call()` | `call_content` handles buffer routing; `call` is the pure logic layer |

## Strong Defaults

| Default | When to break it |
|---|---|
| `OutputGuard::Exploring` (200 item cap) | Use `detail_level: "full"` when you need all items |
| LSP for symbol resolution | Use AST tools (`ListFunctions`, `ListDocs`) for offline/no-LSP scenarios |
| Remote embeddings (Ollama) | Use `local-embed` feature when no Ollama available |
| `read_only: true` for non-home project activation | Pass `read_only: false` when writes are needed |
