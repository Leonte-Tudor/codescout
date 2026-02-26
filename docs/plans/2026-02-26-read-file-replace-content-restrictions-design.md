# Design: Source-file restrictions for `read_file` and `replace_content` removal

**Date:** 2026-02-26
**Status:** Approved

## Problem

Claude falls back to `read_file` (reading source code in chunks) and `replace_content`
(text find-replace on source files) instead of using symbol-level navigation and editing
tools. This bypasses code-explorer's core value proposition: token-efficient, structure-aware
code intelligence.

The routing plugin hooks block the builtin `Read`/`Edit` tools for source files, but
code-explorer's own `read_file` and `replace_content` remain unrestricted, creating a
bypass path.

## Design

### Change 1: Remove `replace_content` entirely

Delete the tool from code-explorer. Rationale:
- For source code files: symbol tools (`replace_symbol_body`, `insert_before_symbol`,
  `insert_after_symbol`) and `edit_lines` are better alternatives
- For non-code files: Claude Code's builtin `Edit` tool handles this
- The routing plugin already blocks it for source files via `edit-router.sh`

**Files:**
- `src/tools/file.rs` — delete `ReplaceContent` struct, `impl Tool`, and all tests
- `src/server.rs` — remove `Arc::new(ReplaceContent)` from tool registration
- `src/util/path_security.rs` — remove `"replace_content"` from `check_tool_access` write-tool arm
- `src/prompts/server_instructions.md` — remove from Editing section and rule 7
- Test in `server_registers_all_tools` — remove `"replace_content"`

**Downstream (routing plugin, separate repo):**
- Delete `hooks/edit-router.sh`
- `hooks/hooks.json` — remove the `replace_content` matcher block
- `hooks/guidance.txt` — remove `replace_content` from EDIT section

### Change 2: Restrict `read_file` to non-source files

Gate `read_file` using `ast::detect_language()`. If the resolved path is recognized as
source code, return a `RecoverableError` with a hint to use symbol tools.

**Gate mechanism:** `ast::detect_language(&resolved)` returns `Some(lang)` for 20+ extensions.
If it returns `Some(lang)` and `lang != "markdown"`, the file is source code → block.
Markdown is detected by `detect_language` (for tree-sitter parsing) but is documentation,
not navigable source code. All other detected languages are source code.
No new extension list to maintain — reuses the existing mapping.

**Why RecoverableError:** Claude sees the problem and a corrective hint, but sibling
parallel tool calls are NOT aborted. This is the established pattern for input-driven
failures.

**Error response:**
```json
{
  "error": "read_file is not available for source code files",
  "hint": "Use symbol tools instead:\n  get_symbols_overview(path) — see all symbols + line numbers\n  find_symbol(name, include_body=true) — read a specific symbol body\n  list_functions(path) — quick function signatures"
}
```

**Files:**
- `src/tools/file.rs` — add gate in `ReadFile::call()` after path resolution (~5-10 lines)
- `src/prompts/server_instructions.md` — update `read_file` description
- New tests: source file blocked, non-source file allowed, unknown extension allowed

### What stays unchanged

- `edit_lines` — kept for intra-symbol edits where line numbers are known from symbol tools
- `read_file` for non-source files — works exactly as before (config, docs, TOML, JSON, YAML, markdown)
- Routing plugin's `semantic-tool-router.sh` — stays as defense-in-depth for builtin tools

## Test impact

- ~10 tests removed (replace_content)
- ~3 tests added (read_file restriction)
- Tool count: 31 → 30
