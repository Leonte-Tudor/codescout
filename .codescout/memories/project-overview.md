# codescout

## Purpose
Rust MCP server giving LLMs IDE-grade code intelligence: symbol navigation via LSP,
semantic search via embeddings, file ops, markdown editing, git integration, persistent
memory, and shell command execution. Designed for use with Claude Code; a companion
routing plugin (`../claude-plugins/code-explorer-routing/`) enforces that LLMs use
codescout tools rather than raw shell reads on source files.

## Tech Stack
- **Language:** Rust 1.75+ (MSRV enforced in CI)
- **MCP SDK:** `rmcp 1.3` (stdio + SSE + streamable-HTTP transports; elicitation support)
- **LSP:** JSON-RPC clients for 9 languages via `lsp-types 0.97`; Unix socket mux for Kotlin multi-client LSP sharing
- **AST:** `tree-sitter` with grammars for Rust/Python/TypeScript/Go/Java/Kotlin
- **Embeddings:** SQLite via `rusqlite` (bundled) + `sqlite-vec` (vec0 virtual tables, KNN); `remote-embed` (reqwest, Ollama/OpenAI-compatible) or `local-embed` (fastembed ONNX) — feature flags
- **Dashboard:** `axum 0.8` (behind `dashboard` feature, opt-in CLI subcommand)

## Runtime Requirements
- Rust stable >= 1.75
- An LSP server per language used (rust-analyzer, pyright, typescript-language-server, etc.)
- For semantic search: Ollama or compatible embedding API (or `local-embed` feature)
- No required env vars — all config is per-project in `.codescout/project.toml`

## Key Feature Flags
- `default`: remote-embed + dashboard + http
- `local-embed`: ONNX-based local embeddings (downloads model ~20-300MB on first use)
- `http`: streamable HTTP transport for multi-session MCP
- `e2e-*`: integration tests requiring real LSP servers installed

## Current Version
v0.8.0 (see `Cargo.toml`). Published to crates.io from `master` branch only.

## 29 Registered Tools
File (7): read_file, list_dir, grep, create_file, glob, edit_file, read_markdown, edit_markdown
Workflow (2): run_command, onboarding
Symbol (9): find_symbol, find_references, goto_definition, hover, list_symbols, replace_symbol, remove_symbol, insert_code, rename_symbol
Semantic (3): semantic_search, index_project, index_status
Memory (1): memory
Config (2): activate_project, project_status
Library (2): list_libraries, register_library
