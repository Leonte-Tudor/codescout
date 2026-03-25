# API Naming Redesign — Parameter and Tool Names

**Date:** 2026-03-25
**Status:** Draft
**Branch:** experiments

## Problem

The codescout API has grown organically to 29 tools. The parameter and tool naming is
implementation-consistent but not user-mental-model-consistent. This matters because the
primary consumers are LLMs — including weak models (Haiku-class) in subagents and
third-party setups with minimal prompt scaffolding.

Key issues:

1. **`name_path` imports filesystem semantics** for a symbol identifier. Every tool that
   takes both `name_path` and `path` forces the LLM to suppress the filesystem meaning
   of "path" for one param and apply it for the other.

2. **`pattern` means three different things** — regex in `search_pattern`, substring in
   `find_symbol`, glob in `find_file`. A weak model has no schema-level signal to
   distinguish these.

3. **`max_results` vs `limit`** — same concept, inconsistent naming across tools. Alias
   logic exists but adds schema noise.

4. **Tool names don't leverage LLM training data.** `search_pattern` and `find_file` are
   descriptive verb phrases, but `grep` and `glob` are universally understood Unix concepts
   that every LLM has seen millions of times.

5. **Markdown logic is embedded in general-purpose tools.** `read_file` has `heading`/
   `headings` params, `edit_file` has `heading` scoping, and `edit_section` is a standalone
   tool. A weak model seeing three tools that can operate on markdown has to learn a
   non-obvious dispatch rule.

### Origin

This design responds to an external API naming review that analyzed all tool schemas and
identified the root problem: the API lacks a stable naming grammar. Parameters like `path`
are overloaded across too many meanings, and adjacent parameter names are underspecified.

### Design Constraint

The only consumers are LLMs reading fresh schemas per MCP session. There is no installed
base of human-written integrations to preserve. This enables a clean break — no aliases,
no deprecation phase, no backward compatibility.

## Design Decisions

1. **Clean break, not phased migration.** No aliases. New names replace old names in a
   single coordinated commit. Rationale: aliases double the schema surface during
   transition, which is exactly the kind of noise that hurts weak models.

2. **`symbol` over `symbol_path`.** The colleague's review recommended `symbol_path`, but
   retaining `path` in the name preserves the exact confusion we're eliminating. The `/`
   separator in values like `MyStruct/my_method` is a format detail that the description
   handles.

3. **Unix-vernacular tool names for search tools.** `grep` and `glob` are terse names that
   every LLM deeply understands from training data. They communicate the search mechanism
   (regex content search, filename pattern matching) more precisely than the descriptive
   verb phrases they replace.

4. **Dedicated markdown tools.** Instead of folding markdown logic into `edit_file`, we
   extract it into dedicated `read_markdown` and `edit_markdown` tools. This gives weak
   models a dead-simple dispatch rule: "is it markdown? Use the markdown tool." Eliminates
   param-based mode switching.

5. **Hard gates on `.md` files.** `read_file` and `edit_file` reject `.md` files with
   `RecoverableError` hints pointing to the markdown tools. Same pattern as the existing
   source code gate that redirects to symbol tools.

## Parameter Renames

| Current | New | Affected Tools | Notes |
|---------|-----|----------------|-------|
| `name_path` | `symbol` | `find_symbol`, `find_references`, `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol` | Description: "Symbol identifier (e.g. 'MyStruct/my_method')" |
| `find_symbol.pattern` | `query` | `find_symbol` only | `grep.pattern` and `glob.pattern` keep `pattern` — tool name disambiguates |
| `max_results` | removed | `grep`, `glob` | Standardize on `limit` only, drop `max_results` and alias logic |
| `project` | `project_id` | `semantic_search`, `memory` | Wherever it appears as a workspace project filter |

### What Does NOT Change

- `path` everywhere — with `name_path` gone, the two-paths confusion is resolved
- `pattern` on `grep` and `glob` — tool names disambiguate
- `scope` — clear enough
- `query` on `semantic_search` — already good
- All payload names: `new_body`, `new_name`, `code`, `content`, `old_string`, `new_string`
- `heading`/`headings` on `read_markdown` and `edit_markdown`
- `json_path`, `toml_key` on `read_file`

### `find_symbol` Dual-Input Behavior

`find_symbol` accepts both `query` (substring search) and `symbol` (exact name path) as
alternatives. When both are provided, `symbol` takes precedence and `query` is ignored —
same as the current `name_path` / `pattern` behavior. The schema description should note:
"Alternative to query. When both are provided, symbol takes precedence."

### Description Updates

Every tool that had `name_path` described with the word "path" gets updated. The new
`symbol` param uses: `"Symbol identifier (e.g. 'MyStruct/my_method')"`. The word "path"
is removed from all descriptions of this parameter.

## Tool Renames

| Current | New | Notes |
|---------|-----|-------|
| `search_pattern` | `grep` | Struct `SearchPattern` → `Grep` |
| `find_file` | `glob` | Struct `FindFile` → `Glob` |
| `edit_section` | `edit_markdown` | Struct `EditSection` → `EditMarkdown`, expanded scope |

### Per-Rename Checklist

Each rename requires updates in:

1. Struct name and `fn name()` return value
2. `Arc::new(...)` in `server.rs::from_parts`
3. `server_registers_all_tools` test
4. `check_tool_access` match arm in `src/util/path_security.rs`
5. Corresponding `*_disabled_blocks_*` security test
6. All three prompt surfaces: `server_instructions.md`, `onboarding_prompt.md`,
   `build_system_prompt_draft()` in `workflow.rs`
7. Error messages and overflow hints that reference tools by name

## `read_markdown` — New Tool

Extracted from `read_file`. Takes over all heading-addressed markdown reading.

### Schema

```
read_markdown(path, heading?, headings?, start_line?, end_line?)
```

- `path` — markdown file path (required)
- `heading` — single heading to read (e.g. `"## Auth"`)
- `headings` — array of headings (mutually exclusive with `heading`)
- `start_line` / `end_line` — line range within a section or the whole file
- No heading params: returns the heading map

### Behavior

Identical to current `read_file` behavior on markdown files. Logic is moved, not changed.

**Non-markdown files:** Returns `RecoverableError` with hint "Use read_file for
non-markdown files." The tool is markdown-only.

**`start_line`/`end_line` with `heading`/`headings`:** These are mutually exclusive
groups, same as current `read_file` behavior. `start_line`/`end_line` operate on the
whole file, not within a resolved section. When called with `start_line`/`end_line`
and no heading params, returns the requested line range (not the heading map).

### `read_file` Changes

- `heading` and `headings` params removed from schema
- When called on a `.md` file: returns `RecoverableError` with hint
  `"Use read_markdown for markdown files"`
- Still works on buffer refs (`@file_*`) since those are plain text

## `edit_markdown` — Expanded from `edit_section`

Absorbs `edit_section`'s functionality plus the `heading`-scoped string replacement
from `edit_file`.

### Schema — Single Mode

```
edit_markdown(path, heading, action, content?, old_string?, new_string?, replace_all?)
```

- `path` — markdown file path (required)
- `heading` — target section (required, fuzzy matching preserved)
- `action` — `replace`, `insert_before`, `insert_after`, `remove`, `edit`
- `content` — for `replace`, `insert_before`, `insert_after` (body only — heading
  preserved on `replace`)
- `old_string`, `new_string`, `replace_all` — for `action="edit"` (scoped string
  replacement within the section)

### Schema — Batch Mode

```
edit_markdown(path, edits)
```

```json
{
  "edits": [
    { "heading": "## Auth", "action": "replace", "content": "new body" },
    { "heading": "## Logging", "action": "edit", "old_string": "foo", "new_string": "bar" },
    { "heading": "## Deprecated", "action": "remove" }
  ]
}
```

Each item in `edits` accepts the same fields as single-mode params minus `path`:
`heading` (required), `action` (required), `content`, `old_string`, `new_string`,
`replace_all`. Validation rules per `action` are identical to single mode.

- `edits` is mutually exclusive with top-level `heading`/`action`/etc.
- Atomic: all or nothing, single write
- Applied in order (earlier edits may shift headings)

### `edit_file` Changes

- `heading` param removed from top-level schema
- `heading` removed from batch `edits` array items schema
- Both removals are complete — the `heading` field becomes unreachable once `.md`
  files are gated, so it is removed from the schema entirely rather than left as dead code
- Heading-scoped string replacement (currently `edit_file(heading=, old_string=, new_string=)`)
  migrates to `edit_markdown(action="edit", heading=, old_string=, new_string=)`
- When called on a `.md` file: returns `RecoverableError` with hint
  `"Use edit_markdown for markdown files"`
- **Exception:** `edit_file` with `insert="prepend"|"append"` on `.md` files is exempted
  from the gate (see Edge Cases)

## Gate Behavior — Markdown Redirect

Same pattern as the existing `edit_file` structural gate (which blocks multi-line
edits with definition keywords on LSP-supported languages). Note: `read_file` no longer
gates source code files — it serves them directly or via buffer. The markdown gate is
a new gate on both `read_file` and `edit_file`.

### Detection

File extension `.md`.

### Responses

| Tool | On `.md` file | Error type | Hint |
|------|--------------|------------|------|
| `read_file` | Blocked | `RecoverableError` | "Use read_markdown for markdown files" |
| `edit_file` | Blocked* | `RecoverableError` | "Use edit_markdown for markdown files" |

\* `edit_file` with `insert="prepend"|"append"` is **exempted** — see Edge Cases.

### Edge Cases

- **Buffer refs** (`@file_*`, `@cmd_*`) — no gate, plain text regardless of original source
- **`read_file` with `mode="complete"` on `.md` plan files** — **exempted from the gate**.
  Plan files use `read_file(mode="complete")` to return the full file inline. Blocking this
  would break plan-reading workflows. The exemption is narrow: only `mode="complete"`.
- **`read_file` with `json_path`/`toml_key` on `.md`** — still blocked (those params don't
  apply to markdown)
- **`edit_file` with `insert="prepend"|"append"` on `.md`** — **exempted from the gate**.
  This is the only `edit_file` mode that passes through on `.md` files. Rationale:
  `edit_markdown` requires a `heading` target, so headingless markdown files or
  file-boundary inserts have no equivalent path. Keeping this escape hatch avoids
  a functionality gap.
- **Surgical edits on headingless `.md` files** — known limitation. `edit_file` is gated,
  `edit_markdown` requires a heading. Workaround: use `edit_file` with `insert="prepend"|"append"`
  for boundary inserts, or `create_file` to rewrite the entire file. This is a narrow gap
  (headingless markdown files that need surgical string replacement) and not worth adding
  complexity to resolve.
- **Files with no extension or ambiguous extensions** — no gate, only `.md`

## File Changes

| File | Changes |
|------|---------|
| `src/tools/symbol.rs` | `name_path` → `symbol` in FindSymbol, FindReferences, ReplaceSymbol, InsertCode, RemoveSymbol, RenameSymbol. `pattern` → `query` in FindSymbol. |
| `src/tools/file.rs` | `SearchPattern` → `Grep`, `FindFile` → `Glob`, drop `max_results` alias, drop `heading`/`headings` from ReadFile schema, drop `heading` from EditFile schema, add `.md` gates |
| `src/tools/semantic.rs` | `project` → `project_id` in SemanticSearch |
| `src/tools/memory.rs` | `project` → `project_id` in memory tool |
| `src/tools/section_edit.rs` | Deleted — logic moves to `markdown.rs` |
| **`src/tools/markdown.rs`** | **New file** — `ReadMarkdown` and `EditMarkdown` structs |
| `src/tools/mod.rs` | Module declaration: remove `section_edit`, add `markdown` |
| `src/server.rs` | Registration updates in `from_parts` (+1 tool: `read_markdown`) |
| `src/util/path_security.rs` | `check_tool_access` match arms updated for new names. **New entry:** `read_markdown` in the read-tools category. `edit_markdown` replaces `edit_section`. |
| `src/prompts/server_instructions.md` | Full pass — tool names, param names, anti-patterns, workflows |
| `src/prompts/onboarding_prompt.md` | Tool name and param references |
| `src/tools/workflow.rs` | `build_system_prompt_draft()` tool references |
| Error messages and overflow hints | Any `RecoverableError` or overflow hint referencing old tool names (`search_pattern`, `find_file`, `edit_section`) |
| Tests across all affected files | Schema names, param names, tool registration, security gates |

## Testing

### Existing Tests to Update

- `server_registers_all_tools` — new tool names, +1 for `read_markdown`
- All `*_disabled_blocks_*` security tests — new tool names
- Tests constructing params with `name_path`, `max_results`, or `pattern` on `find_symbol`
- `edit_section` tests migrate to `edit_markdown` tests

### New Tests

| Test | Verifies |
|------|----------|
| `read_file_blocks_markdown` | `.md` file → `RecoverableError` with hint |
| `edit_file_blocks_markdown` | `.md` file → `RecoverableError` with hint |
| `read_file_allows_md_buffer_refs` | `@file_*` from `.md` source still works |
| `read_markdown_heading_map` | No heading params → returns heading map |
| `read_markdown_single_heading` | `heading=` works |
| `read_markdown_multi_heading` | `headings=[]` works |
| `read_markdown_rejects_non_md` | `.rs` file → error |
| `edit_markdown_action_edit` | `action="edit"` with `old_string`/`new_string` scoped to heading |
| `edit_markdown_batch` | `edits=[]` array, atomic, multi-section |
| `edit_markdown_rejects_non_md` | `.rs` file → error |

## Out of Scope

- `path` on any tool — not renaming to `file` or `within`
- `pattern` on `grep` and `glob` — tool names disambiguate
- `scope` — clear enough
- `insert_code` tool name — accurate as-is
- `semantic_search` tool name — already good
- `goto_definition`, `hover`, `list_symbols` — no changes
- `read_file` params (`json_path`, `toml_key`, `start_line`, `end_line`, `mode`) — unchanged
- Backward-compatibility aliases — none
- Docs in `docs/manual/` — updated as part of the commit but no structural doc changes

## Prompt Surface Updates

All three prompt surfaces must be updated in the same commit:

### `src/prompts/server_instructions.md`

- Tool Reference section: rename tools and params throughout
- Anti-Patterns table: add markdown redirect rows, update tool names in existing rows
- How to Choose the Right Tool: update tool names
- Workflows: update tool names and params
- Iron Laws: update tool references
- Rules: update tool references

### `src/prompts/onboarding_prompt.md`

- All tool name and param references

### Cache Impact

A full rename pass across `server_instructions.md` will invalidate prompt caching for
all active MCP sessions. This is unavoidable and acceptable — the rename is a one-time
cost, and sessions pick up the new instructions on next connection.

## Documentation

This is a refactor of existing API surface, not a new feature. No experimental doc page
under `docs/manual/src/experimental/` is required. The existing tool documentation in
`docs/manual/src/` will be updated as part of the implementation commit to reflect new
tool and parameter names.

### `build_system_prompt_draft()` in `src/tools/workflow.rs`

- Generated prompt references to tool names and params
