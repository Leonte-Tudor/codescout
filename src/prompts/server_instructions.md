codescout MCP server: high-performance semantic code intelligence.
Provides file operations, symbol navigation (LSP), AST analysis (tree-sitter),
semantic search (embeddings), and project memory.

**Subagents and spawned agents SHOULD use codescout too.** If you spawn a subagent
or delegate to another agent, instruct it to use codescout tools for all code
navigation and editing — do not fall back to native Read/Grep/Glob/Edit/Write on
source files. For structural code changes, use `replace_symbol`, `insert_code`,
`remove_symbol` — never the host's native Edit tool.

## Iron Laws

These are non-negotiable. Violating the letter IS violating the spirit.

1. **NO `read_file` ON SOURCE CODE.** Use `list_symbols` + `find_symbol(include_body=true)`.
   `read_file` on source returns a summary, not raw content. Symbol tools give you
   structured, token-efficient navigation. `read_file` is for config, markdown, and data files.

2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `replace_symbol`, `insert_code`,
   `remove_symbol`, or `rename_symbol`. `edit_file` is for imports, literals, comments, config.
   Multi-line edits containing definition keywords (`fn`, `class`, `struct`, etc.) on
   LSP-supported languages return a hard error — the tool tells you which symbol tool to use.

3. **NO PIPING `run_command` OUTPUT.** Run the command bare, then query the `@ref` buffer
   in a follow-up: `cargo test` → `grep FAILED @cmd_id`. Never `cargo test 2>&1 | grep FAILED`.
   The buffer system exists to save your context window — use it.

4. **ALWAYS RESTORE THE ACTIVE PROJECT.** After `activate_project` to a different project,
   you MUST `activate_project` back to the original before finishing your task. The MCP server
   is shared state — forgetting to return silently breaks all subsequent tool calls.
   Subagents share the server with their parent — they MUST restore too.

5. **ACTIVATE THE HOME PROJECT WITH WRITE ACCESS AT SESSION START.** At the start of every
   session, call `activate_project(".", read_only: false)`. This ensures write tools work on
   the current working directory — the server may have been left in an unknown state by a
   previous session or subagent.

## How to Choose the Right Tool

### By knowledge level

| You know… | Start with | Then drill with |
|---|---|---|
| **The name** (function, type, symbol) | `find_symbol(pattern)` or `list_symbols(path)` | `find_symbol(name_path, include_body=true)` |
| **The concept** ("how does auth work?") | `semantic_search(query)` | `list_symbols` / `find_symbol` on results |
| **Nothing** (new codebase) | `list_dir(path)` → `list_symbols(file)` | `semantic_search("what does this do")` |
| **A text pattern** (regex, error message) | `search_pattern(pattern)` | `find_symbol` on matched files |
| **A filename** (glob pattern) | `find_file(pattern)` | `read_file` or `list_symbols` on result |
| **All usages/callers of X** | `find_references(name_path, path)` | `find_symbol` on callers |

## Anti-Patterns — STOP if you catch yourself doing these

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("jq '.key' @file_ref")` to query JSON | `read_file(path, json_path="$.key")` | Navigation params > shell buffer queries |
| `edit_file` with multi-line old_string on `.rs`/`.py`/`.ts` | `replace_symbol(name_path, path, new_body)` | Structural edits > fragile string matching |
| `edit_file` to delete a function | `remove_symbol(name_path, path)` | LSP knows the exact range |
| `edit_file` to add code after a function | `insert_code(name_path, path, code, "after")` | Position-aware, no string matching |
| Native Edit/Write on source files | `replace_symbol`, `insert_code`, `edit_file` | codescout tools are LSP-aware; native tools bypass all safety gates |
| `run_command("cd /abs/path && cmd")` | `run_command("cmd")` — already in project root | Use `cwd` param for subdirectories |
| Repeat a broad `find_symbol` after overflow | Narrow with `path=`, `kind=`, or more specific pattern | Follow the overflow hint |
| Ignore `by_file` in overflow response | Use top file from `by_file` as `path=` filter | The hint tells you exactly where to look |
| `activate_project` for a single lookup | Pass `project: "<id>"` on the tool call | No state mutation, no risk of forgetting to return |
| `edit_file` / `create_file` to rewrite an entire markdown section | `edit_section(path, heading, action, content)` | Heading-addressed, no string matching needed |
| `search_pattern("fn_name")` to find all callers | `find_references(name_path, path)` | LSP finds actual usages; regex matches comments, strings, partial names |

**If you catch yourself rationalizing** ("I'll just quickly read the file", "this edit is
too small for replace_symbol", "one pipe won't hurt") — that's the signal to stop and
use the right tool. Small shortcuts compound into large context waste.

## Tool Reference

### File I/O

- `read_file(path)` — read a file. Large files return a summary + `@file_*` ref.
  Navigate markdown: `read_file(path)` for heading map, then `heading=` or `headings=[]`
  for targeted sections. Other formats: `json_path=` (JSON),
  `toml_key=` (TOML/YAML), or `start_line`/`end_line` for excerpts.
  Auto-chunked: follow the `next` field to continue reading.
  Prefer `list_symbols`/`find_symbol` over `read_file` for source code.
- `list_dir(path)` — list files and directories. Pass `recursive=true` for a full tree.
- `search_pattern(pattern)` — regex search across files. Pass `context_lines` for
  merged context blocks. Scope with `path=`, limit with `max_results` (default 50).
- `find_file(pattern)` — glob-based file search (e.g. `**/*.rs`, `src/**/mod.rs`).
- `create_file(path, content)` — create or overwrite a file.
- `edit_file(path, old_string, new_string)` — exact string replacement. Whitespace-sensitive.
  `replace_all=true` for all occurrences. `insert="prepend"|"append"` to add at file
  boundaries. `heading=` scopes matching to a markdown section (markdown only).
  `edits=[{old_string, new_string}, ...]` for batch operations (atomic, one write).
  For imports, literals, comments, config — NOT structural code changes.
- `edit_section(path, heading, action, content?)` — whole-section operations on markdown.
  Actions: `replace` (pass body only — heading is preserved automatically),
  `insert_before`, `insert_after`, `remove`.
  `heading` uses fuzzy matching (strips inline formatting, prefix/substring fallback).
  Use `edit_section` to replace/insert/remove entire sections; use `edit_file(heading=)` for
  surgical string replacements within a section.
- `read_file` with `mode="complete"` returns entire plan file inline with a delivery receipt.
  Only for files in `plans/` directories. Prefer heading map + `headings=[]` for targeted
  reads — use `mode="complete"` only when you truly need the full plan.

### Symbol Navigation (LSP)

- `find_symbol(pattern)` — locate by name substring. Accepts `name_path` (e.g.
  `MyStruct/my_method`). Filter with `kind`: function, class, struct, interface, type,
  enum, module, constant. Pass `include_body=true` to read the implementation.
- `list_symbols(path)` — symbol tree for file/dir/glob. Pass `include_docs=true` for
  docstrings. Signatures always included. Single-file mode caps at 100 top-level symbols.
- `find_references(name_path, path)` — find all usages of a symbol.
- `goto_definition(path, line)` — jump to definition via LSP. Auto-discovers libraries.
- `hover(path, line)` — type info and documentation for a symbol at a position.

### Symbol Editing (LSP)

- `replace_symbol(name_path, path, new_body)` — replace entire symbol body.
  `new_body` must include the full declaration: attributes, doc comments, signature,
  and body — matching what `find_symbol(include_body=true)` returns.
- `insert_code(name_path, path, code, position)` — insert before or after a named symbol.
- `remove_symbol(name_path, path)` — delete a symbol (removes lines covered by LSP range).
- `rename_symbol(name_path, path, new_name)` — rename across the codebase via LSP.
  Sweeps for textual remainders in comments/docs/strings. **Warning:** may corrupt string
  literals containing the old name — verify compilation after use.

### Semantic Search

- `semantic_search(query)` — find code by natural language or snippet. Returns ranked
  chunks with similarity scores. Use `scope="lib:<name>"` for library code.
- `index_project` — build or update the semantic index. Use `scope="lib:<name>"` to
  index a registered library. Pass `force=true` to rebuild from scratch.

### Workflow

- `run_command(command)` — execute a shell command from the project root. Large output
  stored as `@cmd_*` buffer ref. Stderr captured automatically.
  - `cwd` — run from a subdirectory (relative to project root)
  - `acknowledge_risk` — bypass safety check for destructive commands
  - `timeout_secs` — max execution time (default 30)
  - `run_in_background` — detach and return immediately with a `@bg_*` handle. The
    process runs independently; stdout+stderr go to a log file. A 5-second warm window
    captures startup output. Query later with `run_command("tail -50 @bg_xxx")`. Use
    for dev servers, watchers, or any command that would outlive the timeout.
  - `interactive` — drive an interactive process (REPL, setup wizard) via elicitation
    prompts. Fill in forms with your choice, or leave blank and press enter to cancel.
- `onboarding` — project discovery: detect languages, read key files, generate system
  prompt draft. Use `force=true` to re-scan.

### Memory

- `memory(action, ...)` — persistent project knowledge.
  - `action="write"` — requires `topic`, `content`. Pass `private=true` for gitignored store.
  - `action="read"` — requires `topic`. Pass `private=true` for private store. Pass `sections: ["Rust", "TypeScript"]` to return only the listed `### Heading` blocks (case-insensitive).
  - `action="list"` — pass `include_private=true` to see both shared and private topics.
  - `action="delete"` — requires `topic`. Pass `private=true` for private store.
  - `action="remember"` — store a semantic memory. Requires `content`. Optional `title`.
    Specify `bucket`: `code` | `system` | `preferences` | `unstructured` (default).
  - `action="recall"` — search memories by meaning. Requires `query`. Optional `bucket` filter, `limit`.
  - `action="forget"` — delete a semantic memory. Requires `id` (from recall results).
  - `action="refresh_anchors"` — re-hash anchored files without changing memory content. Use after reviewing a stale memory and confirming it's still accurate. Requires `topic`.
  - **Multi-project workspaces**: Pass `project: "<id>"` to scope operations to a specific project. Omit to use workspace-level memories. Example: `memory(action: "read", project: "backend", topic: "architecture")` 

### Project & Libraries

- `activate_project(path, read_only?)` — switch active project root. Returns an orientation
  card: project name, languages, available memories, semantic index status, and workspace
  siblings. RW activations additionally include security profile and shell toggles.
  Non-home projects default to `read_only: true`. Pass `read_only: false` to enable writes.
  Required after `EnterWorktree`. Use `project_status()` for detailed health checks and
  memory staleness.
- `project_status` — project state: config, semantic index health, usage telemetry,
  library summary, memory staleness. Pass `threshold` for drift scores, `window` for
  usage time range.
- `list_libraries` — registered libraries and index status. Shows version, indexed state,
  and whether the indexed version differs from the current lockfile version (staleness).
  Use `scope="lib:<name>"` in `semantic_search`, `find_symbol`, or `index_project` to target a library.
- `register_library(path, name?, language?)` — manually register an external library.
  Auto-detects name and language from manifest files.

**Library rules:** Pass `scope="lib:<name>"` on `find_symbol`, `list_symbols`,
`find_references`, `semantic_search`, or `index_project` to target a registered library.
Libraries are auto-discovered when `goto_definition`/`hover` resolves outside the project
root. All read-only tools work on libraries; write tools are project-only.


## Output System

**File paths in tool output are relative to the project root** (e.g. `src/tools/mod.rs`,
not `/home/user/project/src/tools/mod.rs`). Pass them as-is to other tools.

### Modes

Default: **exploring** — compact, capped at 200 items.
Pass `detail_level: "full"` for focused mode with `offset`/`limit` pagination.
Only switch to focused AFTER identifying targets.

Overflow produces: `{ "overflow": { "shown": N, "total": M, "hint": "...", "by_file": [...] } }`
— **follow the hint.** Narrow with `path=`, `kind=`, or a more specific `pattern`.
`by_file` shows per-file match counts; use the top file as your `path=` filter.

### Output Buffers

Large content is stored in an `OutputBuffer`. When a result is buffered you receive an
`output_id` field (or `file_id` for large file reads) containing a `@ref` handle.
The full content costs nothing to hold — query it on demand.

#### Buffer ref types and access

| Signal | Ref | Content | Access |
|---|---|---|---|
| `"output_id": "@cmd_abc"` from `run_command` | `@cmd_*` | plain text | `grep pattern @cmd_abc` or `read_file("@cmd_abc", start_line=N)` |
| `"file_id": "@file_abc"` from `read_file` | `@file_*` | plain text | `grep pattern @file_abc` or `read_file("@file_abc", start_line=N)` |
| `"output_id": "@tool_abc"` from other tools | `@tool_*` | JSON | `read_file("@tool_abc", json_path="$.field")` or `start_line`/`end_line` |
| `"output_id": "@bg_abc"` from `run_in_background` | `@bg_*` | plain text | `tail -50 @bg_abc` or `grep pattern @bg_abc` |

**Response fields for `read_file`:**
- `complete: bool` — true if all requested content was returned inline; false if more is available via `next`
- `next: string` — the exact `read_file(...)` call to get the next chunk (only present when `complete: false`)
- `shown_lines: [start, end]` — the original file line numbers of the content shown (present in auto-chunked responses)

**Key distinction:** `@file_*`, `@cmd_*`, `@bg_*` are plain text — grep/sed work directly.
`@tool_*` is JSON — use `json_path` (e.g. `$.symbols[0].body`) or `start_line`/`end_line`.
Don't grep `@tool_*` for code — bodies are JSON strings, not raw text.

**Buffer queries** return ≤ 100 lines inline. Truncation hints show the exact `sed` command
to continue.

## Project Management

### Worktrees

After `EnterWorktree`, call `activate_project` with the worktree path — write tools are
NOT automatically coupled to the shell's working directory. If you forget, writes silently
modify the main repo. To clean up: `git worktree prune` from the main repo root.

### Security Profiles

The project's security profile is set in `.codescout/project.toml`:

- `profile = "default"` (default) — standard sandbox: read deny-list active, writes
  restricted to project root + temp dir, dangerous commands require `acknowledge_risk`.
- `profile = "root"` — unrestricted: no read deny-list, writes allowed anywhere,
  dangerous commands execute without speed bump. For system-administration projects
  that need full filesystem access.

## Workflows

Multi-tool chains for common tasks. Follow the steps in order.

### Editing a Markdown Document

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `read_file(path)` | Get heading map — see all sections |
| 2 | `read_file(path, headings=[...])` | Read target sections (one call, multiple sections) |
| 3a | `edit_section(path, heading, action, content)` | Whole-section: replace (body only — heading preserved), insert, remove |
| 3b | `edit_file(path, heading=, old_string, new_string)` | Surgical: string replacement scoped to a section |
| 3c | `edit_file(path, edits=[...])` | Batch: multiple edits across sections, atomic |

### Impact Analysis — "What breaks if I change X?"

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `find_symbol(name, include_body=true)` | Read current implementation |
| 2 | `find_references(name_path, path)` | Find all callers and dependents |
| 3 | `hover` on key call sites | Reveal concrete types (especially generics/traits) |
| 4 | Edit with full knowledge of blast radius | |

### Dependency Tracing — "How does data flow from A to B?"

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `find_symbol(entry_point)` | Locate starting function |
| 2 | `goto_definition` on called functions | Follow the call chain forward |
| 3 | `hover` on parameters/return values | See resolved types at each stage |
| 4 | `find_references` at destination | Confirm which callers reach this point |

### Safe Rename

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `find_references(name_path, path)` | Map all usages before renaming |
| 2 | `rename_symbol(name_path, path, new_name)` | LSP-powered rename across files |
| 3 | `search_pattern(old_name)` | Catch stragglers in comments, strings, docs |
| 4 | `run_command("cargo check")` | Verify compilation |


## Language Support — Known Issues

### Kotlin (kotlin-lsp)

kotlin-lsp (JetBrains) has a **single workspace session** limitation: only one
kotlin-lsp process can serve a given project directory at a time. If another
codescout instance or editor is already running kotlin-lsp for the same project,
new instances will fail with:

> "Multiple editing sessions for one workspace are not supported yet"

codescout detects this and fails fast with a clear error. **Workaround:** close
the other session first, or use a single codescout instance for Kotlin projects.

This is a known kotlin-lsp limitation (the "not yet" wording indicates JetBrains
plans to lift it in a future release). codescout will update when this is resolved.

All other supported languages (Rust, Python, TypeScript, JavaScript, Go, Java, C/C++,
C#, Ruby) support multiple concurrent LSP sessions without issues.

## Rules

1. **Exploring mode first.** Only `detail_level: "full"` after you know what you need.
2. **Follow overflow hints.** Narrow with `path=`, `kind=`, or a more specific pattern — don't repeat broad queries.
3. **`run_command` is already in the project root.** Never prefix with `cd /abs/path &&`. Use `cwd` for subdirectories.
4. **Check `features_md` from `onboarding` before suggesting features.** Don't propose work that's already done.
5. **Semantic search for "how does X work?"** Then drill into results with symbol tools.
6. **Read `language-patterns` memory before writing or editing code.** `memory(action="read", topic="language-patterns", sections=["<your language>"])` returns only the patterns for your language. Consult it before code changes or code review.
7. **Symbol edits over `edit_file` for code.** `replace_symbol`, `insert_code`, `remove_symbol` for structural changes. `edit_file` for imports, literals, comments.
