# Cross-Project Gotchas

## Fixture Activation Race (MCP Shared State)

The MCP server has a single active project. During parallel workspace onboarding, subagents
calling `activate_project` to their fixture project may overwrite each other's active state.
Symptoms: `glob`, `list_dir`, and `list_symbols` return results from the wrong project.

**Mitigation:** Subagents should read files immediately after `activate_project` before
other subagents can steal the state, or work from the code-explorer root using explicit paths.
`find_symbol` and `search_pattern` always resolve against the indexed project, not active project.

## kotlin-library Memory Scoping

During onboarding (2026-03-30), the kotlin-library subagent's memories ended up inaccessible
under `project_id="kotlin-library"`. The content appeared in the code-explorer project's
`project-overview` slot instead. Root cause: likely `activate_project` was called but
`memory(action="write")` used the wrong active project as scope.

**Mitigation:** When writing fixture-project memories, always explicitly pass `project_id`
to `memory()`. Verify with `memory(action="list", project_id="<id>")` after writing.

## Semantic Index — Fixture Coverage

The semantic index at `.codescout/embeddings/project.db` covers all 6 workspace projects.
To scope semantic search to a specific fixture: `semantic_search(query, project_id="rust-library")`.
Without `project_id`, results may mix code-explorer and fixture content.

## Fixture Libraries Have No Tests

None of the 5 fixture libraries (`java-library`, `kotlin-library`, `python-library`,
`rust-library`, `typescript-library`) have their own test suites. All correctness
validation comes from codescout's `tests/integration.rs` and `tests/symbol_lsp.rs`.
Do not be surprised by the absence of test files in fixtures — it is intentional.

## Book Does Not Implement Searchable in typescript-library

In `typescript-library`, `Book` class does NOT implement the `Searchable` interface.
`Catalog<T extends Searchable>` is therefore not directly usable with `Book` without a wrapper.
This is intentional fixture behavior to demonstrate the constraint without wiring it end-to-end.

## Parallel Write Safety (BUG-021)

Never dispatch parallel `edit_file`, `replace_symbol`, `insert_code`, or `create_file` calls.
rmcp has a cancellation race that can kill the MCP server process. Always sequence writes.
See `MEMORY.md § Parallel Write Safety` for full details.

## Kotlin LSP Multi-Instance Conflict

Multiple concurrent Kotlin LSP instances on the same git repo fight over `.app.lock`.
Use `--system-path` per instance if conflicts arise. See `memory(topic="gotchas", sections=["LSP"])`.
