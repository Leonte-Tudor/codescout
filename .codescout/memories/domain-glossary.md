# Domain Glossary

## codescout Core Terms

| Term | Meaning |
|------|---------|
| `Tool` trait | Core abstraction: `name/description/schema/call/call_content/format_compact` |
| `ToolContext` | Per-call context: agent, lsp, output_buffer, progress, peer, section_coverage |
| `OutputGuard` | Enforces progressive disclosure caps across all tools |
| `RecoverableError` | Expected user-fixable failures → `isError: false` in MCP response |
| `ActiveProject` | Loaded project state accessed via `ctx.agent.with_project()` |
| `LspProvider` / `LspClientOps` | LSP abstraction; `MockLspClient` for tests |
| `OutputBuffer` | Session-scoped LRU buffer (50 entries) for `run_command` results; `@cmd_xxxx` refs |
| `ONBOARDING_VERSION` | Bumped when prompt surfaces change; triggers auto-refresh for stale onboardings |
| `is_subagent_capable` | Gates parallel workspace onboarding dispatch (only for "claude" clients) |
| `build_per_project_prompt` | Generates scoped onboarding prompt for one workspace project |
| `build_synthesis_prompt` | Generates workspace-level memory synthesis instructions |
| `build_workspace_instructions` | Generates workspace dispatch instructions returned by `onboarding` tool |
| `project_prompts` | Array of `{id, path}` in onboarding response for per-project prompt files |
| `synthesis_prompt_path` | Path to workspace synthesis prompt in onboarding response |
| `SecurityProfile` | `default` or `root` — `root` disables all path/command safety gates |
| `SectionCoverage` | Tracks which sections of onboarding prompts are read; included in ToolContext |

## Fixture / Test Domain Terms

| Term | Meaning |
|------|---------|
| `Book` | Core domain entity in all fixture libraries; models a library book with isbn, title, genre, availability |
| `Genre` | Category enum in all fixtures; 5 values: Fiction, NonFiction, Science, History, Biography |
| `Catalog<T>` | Generic service bounded to `Searchable`; add/search/stats |
| `Searchable` | Interface/trait/ABC requiring `search_text()` with default `relevance()` |
| `SearchResult` | Sum type (Found/NotFound/Error) for search operations; sealed in Java/Kotlin, enum in Rust, union in TS |
| `fixture` | A minimal codebase in `tests/fixtures/` used as an external test target for codescout |
| `symbol_lsp tests` | Integration tests in `tests/symbol_lsp.rs` that exercise LSP symbol navigation against fixtures |
| `three-query sandwich` | Cache-invalidation test pattern: baseline → stale assert → flush → fresh assert |
