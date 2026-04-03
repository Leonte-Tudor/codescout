## Cross-Project Terms

| Term | Meaning |
|------|---------|
| **MCP** | Model Context Protocol — the wire protocol codescout implements for LLM-to-tool communication |
| **Tool** | An MCP-callable function (29 registered); each implements the `Tool` trait |
| **OutputGuard** | Progressive disclosure enforcer — exploring (compact) vs focused (full) modes |
| **RecoverableError** | Expected failure that doesn't kill sibling tool calls (isError: false) |
| **OutputBuffer** | Session-scoped LRU storing large results as `@ref` IDs for later querying |
| **Fixture** | One of the 5 language-specific test libraries sharing the same catalog domain |
| **Searchable** | The shared interface/trait across all fixtures — `search_text()` + `relevance()` |
| **Catalog** | Generic collection type in all fixtures — bounded to Searchable, with add/search/stats |
| **Progressive Disclosure** | Design principle: compact output by default, details on demand via detail_level/pagination |
| **Exploring/Focused** | The two output modes enforced by OutputGuard across all tools |
| **vec0** | SQLite virtual table extension (sqlite-vec) used for KNN embedding search |
| **@ref** | Buffer reference ID (e.g., `@cmd_xxxx`, `@tool_xxxx`) for querying stored output |
| **LSP Mux** | Unix socket multiplexer for sharing a single Kotlin LSP server across concurrent sessions |