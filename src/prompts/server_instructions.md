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
   is shared state — forgetting to return silently breaks all subsequent tool calls for the
   parent conversation. Subagents are especially dangerous: they share the server with their
   parent. For quick cross-project lookups (1–3 calls), pass `project: "<id>"` instead of
   switching.

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

**If you catch yourself rationalizing** ("I'll just quickly read the file", "this edit is
too small for replace_symbol", "one pipe won't hurt") — that's the signal to stop and
use the right tool. Small shortcuts compound into large context waste.

## Tool Reference

### File I/O

- `read_file(path)` — read a file. Large files return a summary + `@file_*` ref.
  Navigate large files by format: `heading=` (Markdown), `json_path=` (JSON),
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
  boundaries. For imports, literals, comments, config — NOT structural code changes.

### Symbol Navigation (LSP)

- `find_symbol(pattern)` — locate by name substring. Accepts `name_path` (e.g.
  `MyStruct/my_method`). Filter with `kind`: function, class, struct, interface, type,
  enum, module, constant. Pass `include_body=true` to read the implementation. **Note:** When `path` is
  explicitly specified, the `scope` parameter is ignored — the explicit path takes precedence. Scope only
  affects searches when no path is given (or path is `"."`). Pass `scope="lib:<name>"` to search in a
  registered library.
- `list_symbols(path)` — symbol tree for file/dir/glob. Pass `include_docs=true` for
  docstrings. Signatures always included. Single-file mode caps at 100 top-level symbols.
  Pass `scope="lib:<name>"` to list symbols in a registered library.
- `find_references(name_path, path)` — find all usages of a symbol. Pass `scope="lib:<name>"`
  to search library code. **Note:** Scope filtering is limited to references the project's
  LSP server already knows about.
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
  - `action="read"` — requires `topic`. Pass `private=true` for private store.
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
  library summary. Pass `threshold` for drift scores, `window` for usage time range.
  `memory_staleness` section shows which memories have stale path anchors:
  - `stale` — memories where anchored source files changed since last write
  - `fresh` — memories with all anchors matching current files
  - `untracked` — memories without anchor sidecars
  When memories are stale, review them and either update the memory content (re-writes anchors) or use `memory(action="refresh_anchors", topic="...")` to acknowledge "still accurate."
- `list_libraries` — registered libraries and index status. Shows version, indexed state,
  and whether the indexed version differs from the current lockfile version (staleness).
  Use `scope="lib:<name>"` in `semantic_search`, `find_symbol`, or `index_project` to target a library.
- `register_library(path, name?, language?)` — manually register an external library.
  Auto-detects name and language from manifest files (Cargo.toml, package.json, pyproject.toml, go.mod).
  After registering, use `scope="lib:<name>"` in symbol/search tools, and `index_project(scope="lib:<name>")`
  for semantic search.

**Library rules:**
- Auto-discovered when `goto_definition`/`hover` resolves outside project root.
- All read-only tools work on registered libraries. Write tools are project-only.
- `scope="lib:<name>"` in `semantic_search`/`find_symbol`/`index_project` to target a library.
- Staleness hints appear when lockfile version ≠ indexed version — re-run `index_project`.


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

**Key distinction:** `@file_*` and `@cmd_*` contain plain text — grep/sed are the natural
tools. `@tool_*` contains a JSON object (pretty-printed, multi-line) — use `json_path` to
extract a specific field, or `start_line`/`end_line` to browse sections.

**`@tool_*` json_path examples:**
- `find_symbol` result → `json_path="$.symbols[0].body"` extracts the function body as plain text
- `list_symbols` result → `json_path="$.files[0].symbols"` extracts the symbol list
- Any result → browse with `start_line=1, end_line=50` to see the structure first

**Buffer queries via `run_command`** return ≤ 100 lines inline. Truncation hints show the
exact `sed` command to continue. Do NOT pipe buffer queries — run targeted commands directly.

**Never grep a `@tool_*` ref for code content** — function bodies are JSON string values
(`"body": "fn foo() {\n..."`); grep on code inside them will not work. Use
`json_path="$.symbols[N].body"` instead, or read the source file directly using
`start_line`/`end_line` from the symbol metadata.

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

Source-file shell access guidance (use `read_file`/`find_symbol` instead of `cat`) is
active in both profiles — it improves tool output quality, not security.

### Project Customization

If `.codescout/system-prompt.md` exists, its contents appear below as "Custom
Instructions" — project-specific guidance. Edit the file to customize AI behavior.

## Rules

1. **Exploring mode first.** Only `detail_level: "full"` after you know what you need.
2. **Follow overflow hints.** Narrow with `path=`, `kind=`, or a more specific pattern — don't repeat broad queries.
3. **`run_command` is already in the project root.** Never prefix with `cd /abs/path &&`. Use `cwd` for subdirectories.
4. **Check `features_md` from `onboarding` before suggesting features.** Don't propose work that's already done.
5. **Semantic search for "how does X work?"** Then drill into results with symbol tools.
6. **Read `language-patterns` memory before writing or editing code.** `memory(action="read", topic="language-patterns")` contains per-language anti-patterns and correct patterns. Consult it before code changes or code review.
7. **Symbol edits over `edit_file` for code.** `replace_symbol`, `insert_code`, `remove_symbol` for structural changes. `edit_file` for imports, literals, comments.
