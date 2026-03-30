# Domain Glossary

**OutputGuard** — Progressive disclosure controller. Reads `detail_level`, `offset`, `limit`
from tool input and enforces item/file caps. See `src/tools/output.rs` and
`docs/PROGRESSIVE_DISCOVERABILITY.md`.

**OverflowInfo** — JSON shape emitted when results are capped: `{ shown, total, hint,
next_offset?, by_file?, by_file_overflow }`. The `by_file` array is always JSON array
(not object). See `docs/TODO-by-file-serialization.md`.

**RecoverableError** — Tool error that routes to `isError: false` so Claude Code does
not abort sibling parallel calls. See `src/tools/mod.rs` and `CLAUDE.md § Key Patterns`.

**OutputBuffer** — Session-scoped LRU buffer (50 slots) for large tool output. Assigns
`@tool_xxx` ref IDs. Separate from `@cmd_xxx` refs (run_command) and `@file_xxx` refs
(read_file). See `src/tools/output_buffer.rs`.

**ActiveProject** — Struct inside `Agent` holding the project root, config, both memory
stores, library registry, and dirty file tracking. All tools access via
`ctx.agent.with_project(|p| ...)`.

**LspProvider / LspClientOps** — Traits in `src/lsp/ops.rs` abstracting LSP access.
`LspManager` is the production impl; `MockLspProvider` / `MockLspClient` for tests.

**LspManager mux path** — When `config.mux == true` for a language, `get_or_start`
routes to `get_or_start_via_mux()` which connects to a shared LSP process via a Unix
socket in `/tmp/codescout-*-lsp-mux-<hash>/`. Allows multiple codescout instances to
share one LSP without lock conflicts (Unix only).

**StartingCleanup** — RAII guard in `LspManager::do_start` that removes the per-language
barrier from `self.starting` on any exit path, including async cancellation.

**Circuit breaker** — `LspManager::CIRCUIT_BREAKER_MAX_FAILURES` / `CIRCUIT_BREAKER_WINDOW`
constants control how many consecutive LSP startup failures trigger a fast-fail mode. Resets
after the window expires.

**Scope** — Enum controlling which project a symbol/semantic tool searches:
`Project` (default), `Library(name)`, `Libraries` (all registered), `All`.
Parsed from the `scope` string parameter.

**drift** — Cosine distance score between old and new embeddings for a code chunk after
re-indexing. High drift (≥ `staleness_drift_threshold`) triggers memory anchor staleness.

**anchor sidecar** — `.anchors.toml` file alongside each memory topic. Tracks source file
paths referenced in the memory content, with SHA-256 hashes for staleness detection.

**name_path** — Hierarchical symbol identifier: `Struct/method`, `impl Block/method`.
Separator is `/`. Used in `find_symbol(symbol=...)`, `replace_symbol`, `rename_symbol`.

**tool_timeout_secs** — Per-project config controlling how long `call_tool_inner` waits.
Skipped for slow tools (`index_project`, `onboarding`) via `tool_skips_server_timeout`.

**run_gh** — Internal helper in `src/tools/github.rs` that shells to the `gh` CLI.
All GitHub tools use this. `github_repo` is NOT gated; `github_identity/issue/pr/file`
require `security.github_enabled = true` in project.toml.

**SectionCoverage** — Session-scoped struct (`src/tools/section_coverage.rs`) tracking
which markdown headings have been read per file. Injected into every `read_markdown`
response as `unread_hint`. Invalidates when file mtime changes.

**auto_register_deps** — Function in `src/library/auto_register.rs` called by
`activate_project`. Parses manifest files (Cargo.toml, package.json, pyproject.toml,
go.mod, gradle, pom.xml) and registers locally-available deps as libraries.

**classify_bucket** — Keyword heuristic in `src/memory/classify.rs` that auto-classifies
semantic memories into buckets (code/system/preferences/unstructured).

**Workspace / DiscoveredProject** — Structs in `src/workspace.rs`. `Workspace` holds a
`Vec<Project>` and a `focused` project ID. `discover_projects()` scans for manifests up
to `max_depth` dirs deep to build the workspace.

**elicitation** — MCP protocol feature allowing a tool to prompt the user for input
mid-call. Accessed via `ctx.elicit(schema, title, msg)` using `ToolContext::peer`.
Only works when the client supports it (`is_subagent_capable()`).
