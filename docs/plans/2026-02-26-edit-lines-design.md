# Design: `edit_lines` tool

**Date**: 2026-02-26
**Status**: Approved

## Problem

`replace_content` uses find-and-replace semantics (`old` + `new`), which forces
the caller to send the original text for matching — even for pure insertions
where the old text must appear in both `old` and `new`. This is token-wasteful,
fragile (exact text match required), and dangerous (`replace_all: true` default
can hit unintended locations).

The symbol-aware tools (`replace_symbol_body`, `insert_before_symbol`) solve this
for code files via LSP, but there's no efficient editing tool for non-code files
or intra-symbol edits where line numbers are already known.

## Solution

A line-based splice tool: `edit_lines(path, start_line, delete_count, new_text)`.

Positional addressing via line numbers eliminates the need to echo old content.
Splice semantics handle all four operations (replace, insert, delete, append)
without special conventions or off-by-one traps.

## Tool Signature

```
edit_lines(path, start_line, delete_count, new_text)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | File path (relative to project root or absolute) |
| `start_line` | integer | yes | 1-based line number where the edit begins |
| `delete_count` | integer | yes | Lines to remove starting at `start_line` (0 = pure insertion) |
| `new_text` | string | no | Text to insert in place of deleted lines. Omit or `""` for pure deletion. |

### Operations by example

| Operation | Call |
|-----------|------|
| Replace line 5 | `start=5, delete=1, text="new content"` |
| Replace lines 5-7 | `start=5, delete=3, text="new\nstuff"` |
| Insert before line 5 | `start=5, delete=0, text="inserted line"` |
| Delete lines 5-7 | `start=5, delete=3, text=""` |
| Append at end (10-line file) | `start=11, delete=0, text="new last line"` |

## Behavior

1. Validate path via `validate_write_path` (same security as `replace_content`).
2. Read file into lines.
3. Bounds check: `start_line` in `[1, len+1]`, `delete_count` does not extend past EOF.
4. Splice: remove `delete_count` lines at `start_line`, insert `new_text` split on `\n`.
5. Write back preserving trailing newline convention.
6. Return: `{ "status": "ok", "path", "lines_deleted", "lines_inserted", "new_total_lines" }`.

## Edge Cases

| Case | Behavior |
|------|----------|
| `start_line = total_lines + 1`, `delete_count = 0` | Append at end |
| `start_line > total_lines + 1` | Error with file length context |
| `delete_count` extends past EOF | Error with bounds context |
| `new_text` omitted, `delete_count = 0` | No-op, return 0/0 counts |
| Empty file, `start_line = 1`, `delete_count = 0` | Insert at beginning |

## Implementation Location

- `src/tools/file.rs` — alongside `ReplaceContent` (file-level tool, not symbol-aware)
- Registration in `src/server.rs` `from_parts` in the "File tools" section
- Server instructions update in `src/prompts/server_instructions.md`

## Server Instructions Changes

Add to Editing section:
```
- `edit_lines(path, start_line, delete_count, [new_text])` — line-based splice edit.
  Preferred over replace_content for targeted edits when you know line numbers.
```

Add rule:
```
7. **For edits to code files, prefer symbol tools** over `edit_lines` or `replace_content`.
   Use `edit_lines` for non-code files or intra-symbol edits where you already know
   the line numbers from a previous read.
```
