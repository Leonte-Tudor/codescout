# codescout — Code Explorer Guidance

## Entry Points
- `src/server.rs::CodeScoutServer::from_parts` — all tools registered here; start for tool inventory
- `src/tools/mod.rs` — `Tool` trait definition; read before adding or modifying any tool
- `src/agent.rs::Agent::new` — project activation and state wiring
- `src/tools/workflow.rs::Onboarding` — onboarding tool + workspace parallel dispatch logic

## Key Abstractions
- `Tool` trait (`src/tools/mod.rs`) — name/description/schema/call/call_content/format_compact
- `OutputGuard` (`src/tools/output.rs`) — progressive disclosure; every tool with variable output uses it
- `RecoverableError` (`src/tools/mod.rs`) — recoverable vs fatal error routing
- `LspProvider` / `LspClientOps` (`src/lsp/ops.rs`) — LSP abstraction; `MockLspClient` for tests
- `Agent` / `ActiveProject` (`src/agent.rs`) — project state; all tools access via `ctx.agent.with_project()`
- `OutputBuffer` (`src/tools/output_buffer.rs`) — session-scoped LRU for run_command results; `@cmd_xxxx` refs

## Search Tips
- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "strip_project_root"
- Fixture-scoped: `semantic_search("book catalog search", project_id="rust-library")`
- For a specific tool: `list_symbols("src/tools/<category>.rs")` then `find_symbol(name, include_body=true)`
- For LSP flow: `search_pattern("get_or_start", path="src/lsp")`
- Avoid broad terms: "tool", "error", "file" — too many matches

## Language Navigation

### code-explorer (rust)
- Tool implementations: `src/tools/*.rs` — use `list_symbols` + `find_symbol(include_body=true)`
- LSP layer: `src/lsp/` — `list_symbols("src/lsp/client.rs")` for LSP operations
- Security gates: `src/util/path_security.rs` — `check_tool_access`, `validate_write_path`
- Output layer: `src/tools/output.rs`, `src/tools/output_buffer.rs`

### java-library (java) — `tests/fixtures/java-library/`
- `find_symbol("Book", project_id="java-library")` — record, compact ctor
- `find_symbol("SearchResult", project_id="java-library")` — sealed interface hierarchy
- `semantic_search("generic catalog bounded type", project_id="java-library")`

### kotlin-library (kotlin) — `tests/fixtures/kotlin-library/`
- `find_symbol("Catalog", project_id="kotlin-library")` — generic class + extension functions
- `find_symbol("SearchResult", project_id="kotlin-library")` — sealed class hierarchy
- `semantic_search("sealed class search result variants", project_id="kotlin-library")`

### python-library (python) — `tests/fixtures/python-library/`
- `find_symbol("Catalog", project_id="python-library")` — Generic[T] with nested Stats class
- `find_symbol("AudioBook", project_id="python-library")` — multiple inheritance demo
- `semantic_search("abstract base class search protocol", project_id="python-library")`

### rust-library (rust) — `tests/fixtures/rust-library/`
- `find_symbol("Searchable", project_id="rust-library")` — trait with default impl
- `find_symbol("SearchResult", project_id="rust-library")` — enum with mixed variants
- `semantic_search("lifetime annotation borrow", project_id="rust-library")`

### typescript-library (typescript) — `tests/fixtures/typescript-library/`
- `find_symbol("Catalog", project_id="typescript-library")` — generic class with constraint
- `find_symbol("SearchResult", project_id="typescript-library")` — discriminated union
- `semantic_search("TypeScript decorator namespace merging", project_id="typescript-library")`

## Navigation Strategy
1. New task on a tool → `list_symbols("src/tools/<file>.rs")` + `find_symbol(name, include_body=true)`
2. Cross-cutting change → `search_pattern` across `src/` + check all 3 prompt surfaces
3. Bug in symbol editing → read `docs/TODO-tool-misbehaviors.md` first
4. LSP behavior question → `list_symbols("src/lsp/client.rs")` then targeted `find_symbol`
5. Fixture language question → scope `find_symbol`/`semantic_search` with `project_id`

## Project Rules
- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures; `anyhow::bail!` for genuine bugs
- Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding any tool with variable-length output
- When renaming tools: update all 3 prompt surfaces (server_instructions.md, onboarding_prompt.md, workflow.rs)
- GitHub tools shell to `gh` CLI; `sqlite-vec` is active (vec0 virtual tables with KNN search)
- Never dispatch parallel write calls (BUG-021 — see MEMORY.md)

## Workspace Projects
| ID | Root | Languages |
|----|------|-----------|
| code-explorer | `.` | rust |
| java-library | `tests/fixtures/java-library` | java, kotlin |
| kotlin-library | `tests/fixtures/kotlin-library` | kotlin, java |
| python-library | `tests/fixtures/python-library` | python |
| rust-library | `tests/fixtures/rust-library` | rust |
| typescript-library | `tests/fixtures/typescript-library` | typescript, javascript |
