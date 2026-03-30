## Embedding / Semantic Search

- **sqlite-vec status**: `init_sqlite_vec()` and `maybe_migrate_to_vec0()` are implemented
  and called in `src/embed/index.rs`. vec0 virtual tables may be active — call `is_vec0_active()`
  to determine current mode. When inactive, falls back to pure-Rust cosine scan loading ALL
  chunk embeddings into memory per query (perf issue for large indexes).

- **Model mismatch blocks semantic search**: If the embedding index was built with a
  different model than what's configured in `project.toml`, `semantic_search` returns
  an error. Fix: delete `.codescout/embeddings.db` and rebuild with `index_project(force: true)`,
  or update the `[embeddings] model` setting to match.

- **Semantic index behind HEAD**: The `git_sync` warning in `semantic_search` results is
  expected during active development. Results from existing chunks are still valid; only
  newly added code is missing.

## GitHub Tools

- **Requires `gh` CLI AND `github_enabled = true`**: GitHub tools require both the `gh`
  binary on PATH and `security.github_enabled = true` in `.codescout/project.toml`.
  `github_repo` is NOT gated (always allowed). `github_identity`, `github_issue`,
  `github_pr`, `github_file` ARE gated.

## LSP

- **Kotlin LSP multi-instance conflict**: Multiple kotlin-lsp instances on the same
  git repo fight over an `.app.lock` file. Symptoms: persistent `code -32800`
  (RequestCancelled) on every request, even after retries.
  - **v262+** (installed 2026-03-15): Appears to fix multi-instance issues. Use
    `--system-path` per instance if conflicts recur.
  - **`route_tool_error`** catches `-32800` as recoverable (not fatal).

- **Circuit breaker in LspManager**: After `CIRCUIT_BREAKER_MAX_FAILURES` consecutive
  startup failures within `CIRCUIT_BREAKER_WINDOW`, `get_or_start` fast-fails with an error
  until the window expires. Symptom: "circuit-breaker open" error message.

- **LSP cold start latency**: First call for a language spawns the LSP server. This can
  take 1–5 minutes for large projects (Kotlin/Java). The watch-channel barrier deduplicates
  concurrent starts.

- **LSP mux (Unix only)**: Some languages use `get_or_start_via_mux()` routing via a Unix
  socket to a shared LSP process. Enabled when `config.mux == true`. Socket lives in
  `/tmp/codescout-*-lsp-mux-<hash>/`. Multiple codescout instances share one LSP safely.

## run_command Security

- **Source file access blocked in run_command**: `check_source_file_access()` in
  `path_security.rs` blocks commands like `cat src/foo.rs`. Use `read_file` /
  `search_pattern` / `find_symbol` instead.

## Parallel Writes

- **Never dispatch parallel write calls**: See `MEMORY.md § Parallel Write Safety (BUG-021)`.
  rmcp had a cancellation race (fixed in newer versions, but the architectural rule stands).
  Always wait for one write to finish before starting the next.

## OutputBuffer

- **LRU eviction**: OutputBuffer holds 50 entries. In a long session, early `@tool_xxx`
  refs may be evicted. If you get "buffer not found", re-run the original tool call.

## Memory Staleness

- **Stale memory check is opt-in**: `project_status` shows `memory_staleness` with
  `stale`, `fresh`, and `untracked` entries. Only memories with `.anchors.toml` sidecars
  are tracked. Old memories may show as `untracked`.

## Tool Docs Sync

- **CI enforces docs/manual sync**: The `tool-docs-sync` CI job diffs actual tool names
  against `docs/manual/src/tools/*.md`. Adding a tool without updating docs will fail CI.

## Server Instructions Staleness

- **Instructions pre-computed at startup (stdio)**: `build_server_instructions()` runs
  once in `from_parts`. Changes to memories, index status, or system prompt during a
  session are NOT reflected until the server restarts. HTTP/SSE gets fresh instructions
  per connection.

## Rust std::path

- **`Path::file_stem()` does NOT return `None` for dotfiles**: On `.hidden`,
  Rust treats the entire name as the stem — `file_stem()` returns `Some(".hidden")` and
  `extension()` returns `None`. See `src/memory/anchors.rs` for context.

## Rust Lint Attributes

- **`#[expect(lint)]` vs `#[allow(lint)]` (Rust 1.81+)**: `#[expect]` becomes a compiler
  error if the lint does *not* fire. Use `#[allow]` for intentional temporary suppressions;
  `#[expect]` only when the lint is active today.

## MCP Array Parameters — FIXED

- **`optional_array_param` helper** in `src/tools/mod.rs:286` handles MCP clients that
  serialize array params as JSON strings. It tries `as_array()` first, then
  `serde_json::from_str` fallback. All tools using array params should use this helper.
  The raw `.as_array()` bug (gotcha from 2026-03-23) is now resolved.

## SectionCoverage and ToolContext

- **`ToolContext` requires `section_coverage` field**: Tests that construct `ToolContext`
  manually must include `section_coverage: Arc::new(Mutex::new(SectionCoverage::new()))`.
  Omitting it causes a compile error. See `tests/integration.rs` for the canonical pattern.

- **`peer` field for elicitation**: `ToolContext.peer` is `Option<Peer<RoleServer>>`.
  In unit tests it is `None`. Elicitation calls via `ctx.elicit()` silently no-op when
  `peer` is None — test coverage for elicitation paths requires a live MCP session.
