# edit_file Hard Block for Structural Source Edits

**Date:** 2026-03-15
**Status:** Approved
**Branch:** experiments

## Problem

`edit_file` has a "soft block" on multi-line source file edits: it returns a `pending_ack`
handle with a hint suggesting symbol tools (`replace_symbol`, `insert_code`, `remove_symbol`).
The LLM can bypass this via `acknowledge_risk: true` or by re-calling with the `@ack_*` handle.

In practice, LLMs routinely blow through this speed bump — they just set `acknowledge_risk: true`
or use the handle, defeating the purpose entirely. The result is that structural code changes
(rewriting functions, adding new symbols, deleting symbols) are done via fragile string matching
instead of LSP-backed symbol tools.

## Design

### Gate Logic

Replace the soft block with a hard `RecoverableError`. The gate checks four conditions:

1. `old_string` contains `'\n'` (multi-line edit)
2. `is_source_path(path)` returns true (source file by extension)
3. **Language has LSP support** — new check via `has_lsp_config(lang)`
4. **`old_string` contains a definition keyword** from `DEF_KEYWORDS`

All four must be true for the block to fire. This means:

- **Single-line edits** → always pass through
- **Non-source files** (markdown, toml, json) → always pass through
- **Languages without LSP** (php, swift, scala, elixir, haskell, lua, bash) → pass through
  (symbol tools can't work without an LSP server)
- **Multi-line edits without definition keywords** (imports, string literals, comments,
  decorators, match arms) → pass through
- **Multi-line edits with definition keywords on LSP-supported languages** → **hard block**

### Language Tiers

| Tier | Languages | Tree-sitter | LSP | Symbol tools? | Gate enforced? |
|------|-----------|-------------|-----|---------------|----------------|
| Full | rust, python, typescript/tsx, javascript/jsx, go, java, kotlin | Yes | Yes | Yes | **Yes** |
| LSP-only | c, cpp, csharp, ruby | No | Yes | Yes (via LSP `documentSymbol`) | **Yes** |
| No LSP | php, swift, scala, elixir, haskell, lua, bash | No | No | No | No |

**Note on Tier 2 (LSP-only):** Symbol tools (`replace_symbol`, `insert_code`, `remove_symbol`)
use LSP `textDocument/documentSymbol` for discovery, not tree-sitter. Tree-sitter is only a
fallback in `editing_start_line()` when the LSP doesn't provide `range_start_line`. Mature
LSP servers (clangd, OmniSharp, solargraph) do provide full ranges, so symbol tools work for
these languages.

### Unaffected Code Paths

- **`insert: "prepend"/"append"` mode** — this returns early in `EditFile::call()` before
  the gate check. No `old_string` is involved, so the gate does not apply. This is correct
  and intentional.
- **`replace_all: true`** — follows the same gate logic. A `replace_all` multi-line edit
  containing a definition keyword will be blocked. This is acceptable: batch-replacing
  structural definitions is pathological and better handled by multiple `replace_symbol` calls.

### Error Message

When blocked, the tool returns a `RecoverableError` with a targeted hint based on
`infer_edit_hint()`:

- `new_string` is empty → suggests `remove_symbol(name_path, path)`
- `new_string` longer than `old_string` → suggests `insert_code(name_path, path, code, position)`
- Default → suggests `replace_symbol(name_path, path, new_body)`

Note: since the gate pre-confirms a def keyword is present, the function simplifies to a
clean three-way branch (delete / insert / replace). No fallback needed.

Example:
```
error: "multi-line edit contains a symbol definition ('fn ') — use symbol tools for structural changes"
hint: "replace_symbol(name_path, path, new_body) — replaces the symbol body via LSP"
```

### No Bypass

There is no `acknowledge_risk` parameter, no `@ack_*` handle, no bypass mechanism.
If the gate fires, the LLM must use the suggested symbol tool.

For the rare false positive (e.g., a string literal containing `"fn "` on multiple lines),
the correct workaround is `replace_symbol` on the enclosing function — which is the
behavior we want to encourage anyway.

### DEF_KEYWORDS Change

Remove `"type "` from `DEF_KEYWORDS`. It produces too many false positives — `"type "` appears
constantly in comments ("this type of..."), type annotations, generic bounds, etc. The remaining
keywords (`fn`, `def`, `func`, `fun`, `function`, `class`, `struct`, `impl`, `trait`,
`interface`, `enum`) are all unambiguous definition starters that rarely appear outside
structural contexts.

## Code Changes

### `src/lsp/servers/mod.rs`

Add `has_lsp_config(lang: &str) -> bool`:
```rust
pub fn has_lsp_config(lang: &str) -> bool {
    matches!(lang,
        "rust" | "python" | "typescript" | "javascript" | "tsx" | "jsx"
        | "go" | "java" | "kotlin" | "c" | "cpp" | "csharp" | "ruby"
    )
}
```

### `src/tools/file.rs`

1. **Remove** `acknowledge_risk` parsing from `EditFile::call()`
2. **Remove** `@ack_*` handle dispatch block (`if path.starts_with("@ack_")`)
3. **Add** `has_lsp_support(path: &str) -> bool` helper:
   ```rust
   fn has_lsp_support(path: &str) -> bool {
       let p = std::path::Path::new(path);
       crate::ast::detect_language(p)
           .map(|lang| crate::lsp::servers::has_lsp_config(lang))
           .unwrap_or(false)
   }
   ```
4. **Add** `contains_def_keyword(s: &str) -> bool` helper (wraps `DEF_KEYWORDS` check)
5. **Replace** the soft block with:
   ```rust
   if old_string.contains('\n')
       && crate::util::path_security::is_source_path(path)
       && has_lsp_support(path)
       && contains_def_keyword(old_string)
   {
       let hint = infer_edit_hint(old_string, new_string);
       let keyword = DEF_KEYWORDS.iter().find(|kw| old_string.contains(*kw)).unwrap_or(&"");
       return Err(RecoverableError::with_hint(
           format!("multi-line edit contains a symbol definition ({keyword:?}) — use symbol tools for structural changes"),
           hint,
       ).into());
   }
   ```
6. **Simplify** `infer_edit_hint()`: remove the entire `looks_like_import` heuristic (both
   the original and duplicate copy-paste block at lines 1424-1431) — import edits now pass
   through the gate since they lack def keywords, so import detection is dead code. Remove
   all `acknowledge_risk` references. Keep delete/replace/insert/fallback logic.
7. **Remove `"type "` from `DEF_KEYWORDS`** — too many false positives in comments and
   type annotations.

### `src/tools/output_buffer.rs`

Remove all pending-edit infrastructure:
- `PendingAckEdit` struct
- `pending_edits`, `pending_edits_order`, `max_pending` fields from `OutputBufferInner`
- `store_pending_edit()` method
- `get_pending_edit()` method

### `src/tools/workflow.rs`

1. Remove the cross-tool guard that rejects edit-file `@ack_*` handles passed to `run_command`.
2. Update `acknowledge_risk` schema description for `run_command` — remove any mention of
   `@ack_*` handles from `edit_file` (the run_command ack mechanism is separate and stays).

### `src/prompts/server_instructions.md`

- Update Iron Law #2: remove "blocked" language, state that `edit_file` returns an error
  for structural code changes and the LLM must use symbol tools.
- Remove `acknowledge_risk` mention from `edit_file` description (it was never in the schema
  anyway — only mentioned for `run_command`).

### Tests

**Remove** (~5 tests):
- `edit_file_blocking_hint_always_includes_acknowledge_risk`
- `edit_file_import_list_hint_suggests_acknowledge_risk_not_insert_code`
- `edit_file_acknowledge_risk_bypasses_source_check`
- `edit_file_ack_handle_dispatches_deferred_edit`
- `run_command_rejects_edit_file_ack_handle_with_clear_error`

**Update** (~1 test):
- `infer_edit_hint_import_list_suggests_acknowledge_risk` → remove or repurpose

**Add** (~4 tests):
- `edit_file_blocks_def_keyword_on_lsp_language` — multi-line `fn foo()` on `.rs` → error
- `edit_file_passes_non_lsp_language` — multi-line `fn foo()` on `.lua` → ok
- `edit_file_passes_no_def_keyword` — multi-line import edit on `.rs` → ok
- `edit_file_passes_multiline_non_source` — multi-line edit on `.md` → ok

## Impact

- **Net ~-70 lines** of production code (removal of bypass infrastructure)
- **No schema change** (`acknowledge_risk` was never in the public schema)
- **No breaking change for non-structural edits** — imports, string literals, comments,
  single-line edits all continue to work
- **Breaking for LLMs that relied on bypass** — they must now use symbol tools, which is
  the intended behavior
