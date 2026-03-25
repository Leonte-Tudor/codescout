# API Naming Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename parameters and tools across the codescout MCP server for LLM-optimized clarity, extract dedicated markdown tools, and add .md file gates.

**Architecture:** Clean break rename — no aliases, no backward compatibility. Parameter renames (`name_path`→`symbol`, `pattern`→`query`, `max_results`→`limit`, `project`→`project_id`), tool renames (`search_pattern`→`grep`, `find_file`→`glob`, `edit_section`→`edit_markdown`), new `read_markdown` tool extracted from `read_file`, expanded `edit_markdown` absorbing `edit_file`'s heading logic, and `.md` file gates on `read_file`/`edit_file`.

**Tech Stack:** Rust, MCP protocol (rmcp), tree-sitter, LSP

**Spec:** `docs/superpowers/specs/2026-03-25-api-naming-redesign-design.md`

---

### Task 1: Rename `name_path` → `symbol` in symbol.rs

**Files:**
- Modify: `src/tools/symbol.rs`

This is the highest-impact rename. Every `"name_path"` string in schemas, `call()` param extraction, error messages, hints, **and JSON output fields** must become `"symbol"`. Internal Rust field names (`sym.name_path` on `SymbolInfo`) do NOT change — only the JSON-facing strings.

**Important:** This includes `symbol_to_json()` (line ~314) which emits `"name_path"` as an output field in every symbol response. If the output still says `"name_path"` while the input param is `"symbol"`, weak models will try to use the output field name as a param — the exact confusion we're eliminating.

- [ ] **Step 1: Rename in FindSymbol schema and call()**

In `src/tools/symbol.rs`, change these specific locations:

**Schema (line ~870):**
```rust
// OLD:
"name_path": { "type": "string", "description": "Exact path (e.g. 'MyStruct/my_method'). Alternative to pattern." },
// NEW:
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method'). Alternative to query. Takes precedence when both provided." },
```

**call() param extraction (line ~889):**
```rust
// OLD:
.or_else(|| input["name_path"].as_str())
// NEW:
.or_else(|| input["symbol"].as_str())
```

**Error message (line ~893):**
```rust
// OLD:
"Provide 'pattern' (substring search) or 'name_path' (exact path from get_symbols_overview, e.g. 'MyStruct/my_method')",
// NEW:
"Provide 'query' (substring search) or 'symbol' (exact identifier, e.g. 'MyStruct/my_method')",
```

**is_name_path check (line ~903):**
```rust
// OLD:
let is_name_path = input["name_path"].is_string();
// NEW:
let is_name_path = input["symbol"].is_string();
```
(Keep the variable name `is_name_path` — it's internal Rust, not user-facing.)

**Overflow hints (lines ~639, ~655):**
```rust
// OLD:
"find_symbol(name_path='...', include_body=true) for a specific symbol."
// NEW:
"find_symbol(symbol='...', include_body=true) for a specific symbol."
```

```rust
// OLD:
"or find_symbol(name_path='ClassName/methodName', include_body=true) for a specific symbol."
// NEW:
"or find_symbol(symbol='ClassName/methodName', include_body=true) for a specific symbol."
```

**Truncation hint (line ~1195):**
```rust
// OLD:
json!("use find_symbol with name_path for full body"),
// NEW:
json!("use find_symbol with symbol for full body"),
```

- [ ] **Step 2: Rename in FindReferences schema and call()**

**Description (line ~1237):**
```rust
// OLD:
"Find all usages of a symbol. Requires name_path and file."
// NEW:
"Find all usages of a symbol. Requires symbol and file."
```

**Schema (lines ~1242-1244):**
```rust
// OLD:
"required": ["name_path", "path"],
...
"name_path": { "type": "string", "description": "Symbol path (e.g. 'MyStruct/my_method')" },
// NEW:
"required": ["symbol", "path"],
...
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
```

**call() param extraction (line ~1254):**
```rust
// OLD:
let name_path = super::require_str_param(&input, "name_path")?;
// NEW:
let name_path = super::require_str_param(&input, "symbol")?;
```

- [ ] **Step 3: Rename in ReplaceSymbol schema and call()**

**Schema (lines ~1821-1823):**
```rust
// OLD:
"required": ["name_path", "path", "new_body"],
...
"name_path": { "type": "string" },
// NEW:
"required": ["symbol", "path", "new_body"],
...
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
```

**call() (line ~1831):**
```rust
// OLD:
let name_path = super::require_str_param(&input, "name_path")?;
// NEW:
let name_path = super::require_str_param(&input, "symbol")?;
```

- [ ] **Step 4: Rename in RemoveSymbol schema and call()**

**Schema (lines ~1896-1898):**
```rust
// OLD:
"required": ["name_path", "path"],
...
"name_path": { "type": "string", "description": "Symbol name path (e.g. 'MyStruct/my_method', 'tests/old_test')" },
// NEW:
"required": ["symbol", "path"],
...
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method', 'tests/old_test')" },
```

**call() (line ~1906):**
```rust
// OLD:
let name_path = super::require_str_param(&input, "name_path")?;
// NEW:
let name_path = super::require_str_param(&input, "symbol")?;
```

- [ ] **Step 5: Rename in InsertCode schema and call()**

**Schema (lines ~1973-1975):**
```rust
// OLD:
"required": ["name_path", "path", "code"],
...
"name_path": { "type": "string", "description": "Symbol name path (e.g. 'MyStruct/my_method')" },
// NEW:
"required": ["symbol", "path", "code"],
...
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
```

**call() (line ~1988):**
```rust
// OLD:
let name_path = super::require_str_param(&input, "name_path")?;
// NEW:
let name_path = super::require_str_param(&input, "symbol")?;
```

- [ ] **Step 6: Rename in RenameSymbol schema and call()**

**Schema (lines ~2157-2159):**
```rust
// OLD:
"required": ["name_path", "path", "new_name"],
...
"name_path": { "type": "string" },
// NEW:
"required": ["symbol", "path", "new_name"],
...
"symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
```

**call() (line ~2167):**
```rust
// OLD:
let name_path = super::require_str_param(&input, "name_path")?;
// NEW:
let name_path = super::require_str_param(&input, "symbol")?;
```

- [ ] **Step 7: Rename output field in `symbol_to_json()`**

**Line ~314:**
```rust
// OLD:
map.insert("name_path".into(), json!(sym.name_path));
// NEW:
map.insert("symbol".into(), json!(sym.name_path));
```

The internal `sym.name_path` Rust field stays — only the JSON key changes.

- [ ] **Step 8: Update `search_pattern` references in symbol.rs hints**

These error hints reference the old `search_pattern` tool name. Update to `grep`:

**Line ~906:**
```rust
// OLD:
// not regex. Point the LLM to search_pattern instead.
// NEW:
// not regex. Point the LLM to grep instead.
```

**Line ~922:**
```rust
// OLD:
"Use search_pattern(pattern=\"...\") for regex text search, \
// NEW:
"Use grep(pattern=\"...\") for regex text search, \
```

**Lines ~1434, ~1608:**
```rust
// OLD:
"Check the line number — use list_symbols or search_pattern to find correct lines",
// NEW:
"Check the line number — use list_symbols or grep to find correct lines",
```

- [ ] **Step 9: Update test that asserts `search_pattern` in hint**

**Lines ~7674-7675:**
```rust
// OLD:
rec.hint.as_deref().unwrap_or("").contains("search_pattern"),
"hint should mention search_pattern, got: {:?}",
// NEW:
rec.hint.as_deref().unwrap_or("").contains("grep"),
"hint should mention grep, got: {:?}",
```

- [ ] **Step 10: Update tests that pass `"name_path"` as input param**

Search all tests in `symbol.rs` for `"name_path"` in JSON input and update to `"symbol"`. These tests construct tool input params — if the schema no longer accepts `"name_path"`, these tests will silently pass null.

Run: `grep -n '"name_path"' src/tools/symbol.rs | grep -v 'sym.name_path\|name_path\.into'`

Update every test JSON that uses `"name_path": "..."` as a tool input param to `"symbol": "..."`.

- [ ] **Step 11: Verify compilation**

Run: `cargo build 2>&1`
Expected: Compiles successfully. No internal `SymbolInfo.name_path` fields were changed — only JSON-facing strings.

- [ ] **Step 12: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "refactor: rename name_path → symbol in all symbol tool schemas and output"
```

---

### Task 2: Rename `pattern` → `query` in FindSymbol

**Files:**
- Modify: `src/tools/symbol.rs`

Only `FindSymbol` is affected. `SearchPattern.pattern` and `FindFile.pattern` keep their names (they become `grep` and `glob` where tool name disambiguates).

- [ ] **Step 1: Rename in FindSymbol schema**

**Schema (line ~869):**
```rust
// OLD:
"pattern": { "type": "string", "description": "Symbol name or substring to search for" },
// NEW:
"query": { "type": "string", "description": "Symbol name or substring to search for" },
```

- [ ] **Step 2: Rename in FindSymbol call() param extraction**

**Line ~888:**
```rust
// OLD:
let pattern = input["pattern"]
    .as_str()
    .or_else(|| input["symbol"].as_str())  // (already renamed from Step 1)
// NEW:
let pattern = input["query"]
    .as_str()
    .or_else(|| input["symbol"].as_str())
```
(Keep the local variable name `pattern` — it's internal Rust.)

**Error message (already updated in Task 1 Step 1 to reference `query`)** — verify it says:
```rust
"Provide 'query' (substring search) or 'symbol' (exact identifier, e.g. 'MyStruct/my_method')",
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build 2>&1`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "refactor: rename find_symbol.pattern → query"
```

---

### Task 3: Rename `project` → `project_id` in semantic.rs and memory.rs

**Files:**
- Modify: `src/tools/semantic.rs`
- Modify: `src/tools/memory.rs`

- [ ] **Step 1: Rename in SemanticSearch schema and call()**

**Schema (line ~31 in semantic.rs):**
```rust
// OLD:
"project": { "type": "string", "description": "Filter to a project ID." }
// NEW:
"project_id": { "type": "string", "description": "Filter to a workspace project ID." }
```

**call() param extraction (line ~42):**
```rust
// OLD:
.get("project")
// NEW:
.get("project_id")
```

- [ ] **Step 2: Rename in memory.rs schema and call()**

**Schema (line ~457 in memory.rs):**
```rust
// OLD:
"project": { "type": "string", "description": "Scope to a project ID. Default: focused project." }
// NEW:
"project_id": { "type": "string", "description": "Scope to a workspace project ID. Default: focused project." }
```

**call() param extraction (line ~349):**
```rust
// OLD:
let project_param = input.get("project").and_then(|v| v.as_str());
// NEW:
let project_param = input.get("project_id").and_then(|v| v.as_str());
```

- [ ] **Step 3: Update memory.rs tests that pass `"project"` param**

Lines ~1593, ~1629, ~1643, ~1653 — change `"project": "mcp-server"` to `"project_id": "mcp-server"` in test JSON.

- [ ] **Step 4: Update semantic.rs tests that reference `"project"` param**

Line ~1095 — change `props.contains_key("project")` to `props.contains_key("project_id")`.

**Note:** Don't change `"project"` where it's used as a `scope` value (e.g., `scope: "project"`) or as a `source` classification string (e.g., `source: "project"`). Only change the param name in schemas and input extraction.

**Scope confirmed:** `"project"` as a tool input param only appears in `semantic.rs` and `memory.rs` schemas. No other tools have it.

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo build 2>&1`
Run: `cargo test -- semantic memory 2>&1`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/semantic.rs src/tools/memory.rs
git commit -m "refactor: rename project → project_id param in semantic_search and memory"
```

---

### Task 4: Rename SearchPattern → Grep and FindFile → Glob

**Files:**
- Modify: `src/tools/file.rs`

- [ ] **Step 1: Rename SearchPattern struct and tool name**

In `src/tools/file.rs`:
- Rename struct `SearchPattern` → `Grep` (find the struct definition and all references)
- Change `fn name()` return: `"search_pattern"` → `"grep"`
- Rename `format_search_pattern` → `format_grep`
- Update the section comment `// ── search_pattern` → `// ── grep`
- Update any `impl Tool for SearchPattern` → `impl Tool for Grep`

- [ ] **Step 2: Drop `max_results` from Grep schema, keep `limit` only**

**Schema (lines ~894-895):**
```rust
// OLD:
"max_results": { "type": "integer", "default": 50, "description": "Max matching lines. Alias: limit" },
"limit": { "type": "integer", "description": "Alias for max_results" },
// NEW:
"limit": { "type": "integer", "default": 50, "description": "Max matching lines" },
```

**call() param extraction (line ~911):**
```rust
// OLD:
let max = optional_u64_param(&input, "max_results")
    .or_else(|| optional_u64_param(&input, "limit"))
// NEW:
let max = optional_u64_param(&input, "limit")
```

- [ ] **Step 3: Update `infer_edit_hint()` in file.rs to use new param names**

**Lines ~1762-1768 in `infer_edit_hint()`:**
```rust
// OLD:
"remove_symbol(name_path, path) — deletes the symbol and its doc comments/attributes"
"insert_code(name_path, path, code, position) — inserts before or after a named symbol"
"replace_symbol(name_path, path, new_body) — replaces the symbol body via LSP"
// NEW:
"remove_symbol(symbol, path) — deletes the symbol and its doc comments/attributes"
"insert_code(symbol, path, code, position) — inserts before or after a named symbol"
"replace_symbol(symbol, path, new_body) — replaces the symbol body via LSP"
```

- [ ] **Step 4: Update error hints in Grep that reference old name**

**Lines ~1995, ~2077 in EditFile:**
```rust
// OLD:
"Check whitespace and indentation. Use search_pattern to verify exact text.",
"Check whitespace and indentation — old_string must match exactly. Use search_pattern to verify the exact text.",
// NEW:
"Check whitespace and indentation. Use grep to verify exact text.",
"Check whitespace and indentation — old_string must match exactly. Use grep to verify the exact text.",
```

- [ ] **Step 5: Rename FindFile struct and tool name**

- Rename struct `FindFile` → `Glob`
- Change `fn name()` return: `"find_file"` → `"glob"`
- Rename `format_find_file` → `format_glob`
- Update section comment `// ── find_file` → `// ── glob`
- Update `impl Tool for FindFile` → `impl Tool for Glob`

- [ ] **Step 6: Drop `max_results` from Glob schema, keep `limit` only**

**Schema (lines ~1105-1106):**
```rust
// OLD:
"max_results": { "type": "integer", "default": 100, "description": "Maximum files to return. Alias: limit" },
"limit": { "type": "integer", "description": "Alias for max_results" }
// NEW:
"limit": { "type": "integer", "default": 100, "description": "Maximum files to return" },
```

**call() param extraction (line ~1121):**
```rust
// OLD:
let max = optional_u64_param(&input, "max_results")
    .or_else(|| optional_u64_param(&input, "limit"))
// NEW:
let max = optional_u64_param(&input, "limit")
```

- [ ] **Step 7: Update ReadFile hint that references `find_file`**

**Line ~314:**
```rust
// OLD:
"Check the path with list_dir, or use find_file to locate the file",
// NEW:
"Check the path with list_dir, or use glob to locate the file",
```

- [ ] **Step 8: Rename test functions**

Rename all test functions that reference old names:
- `find_file_matches_glob` → `glob_matches_pattern`
- `find_file_recursive_glob` → `glob_recursive`
- `find_file_respects_max_results` → `glob_respects_limit`
- `find_file_no_matches` → `glob_no_matches`
- `find_file_skips_hidden_dirs` → `glob_skips_hidden_dirs`
- Update any test that passes `"max_results"` as a param to use `"limit"` instead.

- [ ] **Step 9: Verify compilation**

Run: `cargo build 2>&1`
Expected: Will fail because `server.rs` still imports `SearchPattern` and `FindFile`. That's expected — we fix it in Task 8.

- [ ] **Step 10: Commit (local only, don't push yet)**

```bash
git add src/tools/file.rs
git commit -m "refactor: rename search_pattern → grep, find_file → glob"
```

---

### Task 5: Create `src/tools/markdown.rs` with ReadMarkdown

**Files:**
- Create: `src/tools/markdown.rs`
- Modify: `src/tools/file.rs` (extract heading logic)

This task extracts the markdown heading-reading logic from `ReadFile` into a new `ReadMarkdown` tool.

- [ ] **Step 1: Create `src/tools/markdown.rs` with ReadMarkdown struct**

Create the file with the `ReadMarkdown` tool. The implementation reuses the same markdown heading logic currently in `ReadFile::call()`. Key sections to extract:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::mod_prelude::*;
use super::RecoverableError;
use crate::tools::{Tool, ToolContext};

pub struct ReadMarkdown;

#[async_trait]
impl Tool for ReadMarkdown {
    fn name(&self) -> &str {
        "read_markdown"
    }

    fn description(&self) -> &str {
        "Read markdown files with heading-based navigation. Returns heading map by default, or content of specific sections."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Markdown file path relative to project root" },
                "heading": { "type": "string", "description": "Markdown section by heading (e.g. \"## Auth\")." },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of headings to read (returns multiple sections). Mutually exclusive with heading."
                },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let path_str = super::require_str_param(&input, "path")?;

        // Gate: only .md files
        if !path_str.ends_with(".md") {
            return Err(RecoverableError::new(
                "read_markdown only supports .md files",
                "Use read_file for non-markdown files.",
            ).into());
        }

        // Delegate to the existing read_file logic for markdown
        // (extract the heading-handling code from ReadFile::call)
        // ... implementation moved from file.rs
    }
}
```

The `call()` body should reuse the markdown-reading logic from `ReadFile`. Key code in `file.rs`:

- **Line ~249:** `let heading = input["heading"].as_str();` — start of navigation param handling
- **Line ~250:** `let headings_param = super::optional_array_param(&input, "headings");` — multi-heading
- **Lines ~255-260:** Mutual exclusivity validation for heading vs headings
- The heading map generation is the default `.md` response when no heading params are provided
- The `format_compact` method on `ReadFile` (in the `section_coverage` module) also handles markdown rendering

The cleanest approach: extract the markdown-specific branch from `ReadFile::call()` into a standalone function (e.g., `read_markdown_impl()`) that `ReadMarkdown::call()` uses directly. Don't duplicate — move.

- [ ] **Step 2: Add the non-heading line-range path**

When `start_line`/`end_line` are provided without `heading`/`headings`, `ReadMarkdown` should return the raw line range from the file (same as `read_file` does today for line ranges). This reuses existing line-range logic.

- [ ] **Step 3: Verify the file compiles standalone**

Run: `cargo build 2>&1` (will fail on missing module declaration — expected)

- [ ] **Step 4: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "feat: add ReadMarkdown tool (extracted from read_file)"
```

---

### Task 6: Expand EditMarkdown from EditSection

**Files:**
- Modify: `src/tools/markdown.rs` (add EditMarkdown)
- Reference: `src/tools/section_edit.rs` (source of logic to move)

- [ ] **Step 1: Move EditSection logic into markdown.rs as EditMarkdown**

Copy the `EditSection` struct and `impl Tool` from `section_edit.rs` into `markdown.rs`, renaming to `EditMarkdown`. Keep all the helper functions (`perform_section_edit`, `compute_section_end`, `join_lines`, etc.) — move them too.

Update:
- Struct name: `EditSection` → `EditMarkdown`
- `fn name()`: `"edit_section"` → `"edit_markdown"`
- Description: update to reflect expanded scope

- [ ] **Step 2: Expand schema with `action="edit"` and batch mode**

Update `input_schema()`:
```rust
fn input_schema(&self) -> Value {
    json!({
        "type": "object",
        "required": ["path"],
        "properties": {
            "path": { "type": "string", "description": "Markdown file path" },
            "heading": { "type": "string", "description": "Target section heading (fuzzy matched)" },
            "action": {
                "type": "string",
                "enum": ["replace", "insert_before", "insert_after", "remove", "edit"],
                "description": "Operation to perform"
            },
            "content": { "type": "string", "description": "New content for replace/insert actions (body only — heading preserved on replace)" },
            "old_string": { "type": "string", "description": "For action='edit': exact text to find within section" },
            "new_string": { "type": "string", "description": "For action='edit': replacement text" },
            "replace_all": { "type": "boolean", "default": false, "description": "For action='edit': replace all occurrences" },
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["heading", "action"],
                    "properties": {
                        "heading": { "type": "string" },
                        "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove", "edit"] },
                        "content": { "type": "string" },
                        "old_string": { "type": "string" },
                        "new_string": { "type": "string" },
                        "replace_all": { "type": "boolean" }
                    }
                },
                "description": "Batch mode: array of edit operations applied atomically. Mutually exclusive with top-level heading/action."
            }
        }
    })
}
```

- [ ] **Step 3: Implement `action="edit"` (heading-scoped string replacement)**

In the `call()` method, add handling for `action="edit"`:
1. Resolve the heading section (same as other actions)
2. Extract the section content
3. Perform `old_string` → `new_string` replacement within that section only
4. If `replace_all`, replace all occurrences; otherwise replace first only
5. Reconstruct the file with the modified section

This is the logic currently in `EditFile::call()` behind the `heading_scope` parameter (file.rs lines ~1967+). Extract and adapt it.

- [ ] **Step 4: Implement batch mode**

In `call()`, check for `edits` array. If present:
1. Validate that top-level `heading`/`action` are NOT also present
2. Read the file once
3. Apply each edit in order (each edit is a single-mode operation)
4. Write the file once (atomic)

- [ ] **Step 5: Add .md gate**

At the top of `call()`:
```rust
if !path_str.ends_with(".md") {
    return Err(RecoverableError::new(
        "edit_markdown only supports .md files",
        "Use edit_file for non-markdown files.",
    ).into());
}
```

- [ ] **Step 6: Move unit tests from section_edit.rs**

Move all tests from `section_edit.rs`'s `mod tests` into `markdown.rs`'s test module. Update test function names to reference `edit_markdown` where they reference `edit_section`.

- [ ] **Step 7: Verify compilation**

Run: `cargo build 2>&1`

- [ ] **Step 8: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "feat: add EditMarkdown with action=edit and batch mode"
```

---

### Task 7: Add .md gates to ReadFile and EditFile, remove heading params

**Files:**
- Modify: `src/tools/file.rs`

- [ ] **Step 1: Add .md gate to ReadFile::call()**

Near the top of `ReadFile::call()`, after resolving the path, add:
```rust
// Gate: redirect .md files to read_markdown
// Exempt: buffer refs (@file_*), and mode="complete" (plan files)
let is_buffer_ref = path_str.starts_with("@");
let is_complete_mode = input["mode"].as_str() == Some("complete");
if resolved_path.extension().map_or(false, |e| e == "md")
    && !is_buffer_ref
    && !is_complete_mode
{
    return Err(RecoverableError::new(
        "Use read_markdown for markdown files",
        "read_markdown provides heading-based navigation for .md files.",
    ).into());
}
```

- [ ] **Step 2: Remove `heading` and `headings` from ReadFile schema**

Remove these lines from `ReadFile::input_schema()` (lines ~34-39):
```rust
// REMOVE:
"heading": { "type": "string", "description": "Markdown section by heading (e.g. \"## Auth\")." },
"headings": {
    "type": "array",
    "items": { "type": "string" },
    "description": "List of headings to read (returns multiple sections). Mutually exclusive with heading."
},
```

- [ ] **Step 3: Remove heading logic from ReadFile::call()**

Remove the code path in `ReadFile::call()` that handles `input["heading"]` and `input["headings"]` — this logic is now in `ReadMarkdown`. The heading-map generation for `.md` files is also handled by the gate (the user will never reach this code for `.md` files).

- [ ] **Step 4: Add .md gate to EditFile::call()**

Near the top of `EditFile::call()`, after resolving the path, add:
```rust
// Gate: redirect .md files to edit_markdown (except prepend/append)
if resolved_path.extension().map_or(false, |e| e == "md") {
    let insert_mode = input["insert"].as_str();
    if insert_mode != Some("prepend") && insert_mode != Some("append") {
        return Err(RecoverableError::new(
            "Use edit_markdown for markdown files",
            "edit_markdown provides heading-based editing for .md files. edit_file with insert='prepend'/'append' is still allowed.",
        ).into());
    }
}
```

- [ ] **Step 5: Remove `heading` from EditFile schema**

Remove from top-level schema (line ~1794):
```rust
// REMOVE:
"heading": { "type": "string", "description": "Scope string matching to a markdown section. Only valid for .md files." },
```

Remove from batch edits items schema (line ~1802):
```rust
// REMOVE from items properties:
"heading": { "type": "string" },
```

- [ ] **Step 6: Remove heading-scoping logic from EditFile::call()**

Remove the `heading_scope` code paths in both single and batch edit modes (lines ~1842, ~1967). This logic has moved to `edit_markdown(action="edit")`.

- [ ] **Step 7: Update tests that use heading on edit_file**

Any existing test in `file.rs` that tests `edit_file` with `"heading"` param now needs to either:
- Be moved to test `edit_markdown` instead, or
- Be converted to test the `.md` gate behavior

Tests that test `edit_file` with `insert="prepend"/"append"` on `.md` should remain and verify the exemption works.

- [ ] **Step 8: Verify compilation**

Run: `cargo build 2>&1`

- [ ] **Step 9: Commit**

```bash
git add src/tools/file.rs
git commit -m "refactor: add .md gates to read_file/edit_file, remove heading params"
```

---

### Task 8: Update server registration, mod.rs, and path_security.rs

**Files:**
- Modify: `src/tools/mod.rs`
- Modify: `src/server.rs`
- Modify: `src/util/path_security.rs`
- Delete: `src/tools/section_edit.rs`

- [ ] **Step 1: Update mod.rs**

```rust
// REMOVE:
pub mod section_edit;
// ADD:
pub mod markdown;
```

- [ ] **Step 2: Update server.rs imports**

```rust
// OLD:
use crate::tools::file::{CreateFile, EditFile, FindFile, ListDir, ReadFile, SearchPattern};
// NEW:
use crate::tools::file::{CreateFile, EditFile, Glob, Grep, ListDir, ReadFile};
```

Add:
```rust
use crate::tools::markdown::{EditMarkdown, ReadMarkdown};
```

Remove:
```rust
// Any import of section_edit::EditSection
```

- [ ] **Step 3: Update server.rs from_parts registration**

```rust
// OLD:
Arc::new(SearchPattern),
...
Arc::new(FindFile),
...
Arc::new(crate::tools::section_edit::EditSection),
// NEW:
Arc::new(Grep),
...
Arc::new(Glob),
...
Arc::new(ReadMarkdown),
Arc::new(EditMarkdown),
```

- [ ] **Step 4: Update check_tool_access in path_security.rs**

```rust
// OLD:
"create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
| "remove_symbol" | "register_library" | "edit_section" => {
// NEW:
"create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
| "remove_symbol" | "register_library" | "edit_markdown" => {
```

`read_markdown` is a read tool — it falls through to the `_ => {}` (always allowed) arm, same as `read_file`, `list_dir`, `grep`, `glob`.

- [ ] **Step 5: Update server_registers_all_tools test**

```rust
let expected_tools = [
    "read_file",
    "list_dir",
    "grep",           // was search_pattern
    "create_file",
    "glob",           // was find_file
    "edit_file",
    "read_markdown",  // NEW
    "edit_markdown",  // was edit_section
    "run_command",
    "onboarding",
    "find_symbol",
    "find_references",
    "list_symbols",
    "replace_symbol",
    "insert_code",
    "rename_symbol",
    "remove_symbol",
    "goto_definition",
    "hover",
    "memory",
    "semantic_search",
    "index_project",
    "index_status",
    "activate_project",
    "project_status",
    "list_libraries",
    "register_library",
];
```

Note: tool count increases by 1 (net: `edit_section` removed, `read_markdown` + `edit_markdown` added). Verify the actual count from the current `server_registers_all_tools` test rather than assuming a specific number.

- [ ] **Step 6: Update security tests**

In `file_write_disabled_blocks_all_write_tools`:
- Remove `"edit_section"` if present, add `"edit_markdown"`

In `read_tools_always_allowed`:
- Change `"search_pattern"` → `"grep"`
- Change `"find_file"` → `"glob"`
- Add `"read_markdown"`

- [ ] **Step 7: Delete section_edit.rs**

```bash
rm src/tools/section_edit.rs
```

- [ ] **Step 8: Verify compilation and all tests**

Run: `cargo build 2>&1`
Run: `cargo test 2>&1`
Expected: All pass.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: update server registration, security gates, delete section_edit.rs"
```

---

### Task 9: Write new tests

**Files:**
- Modify: `src/tools/file.rs` (gate tests)
- Modify: `src/tools/markdown.rs` (new tool tests)

- [ ] **Step 1: Add `read_file_blocks_markdown` test**

In `src/tools/file.rs` tests:
```rust
#[tokio::test]
async fn read_file_blocks_markdown() {
    // Create a .md file, call ReadFile, expect RecoverableError with hint
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Hello\n\nworld").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = ReadFile.call(json!({"path": "test.md"}), &ctx).await;
    // Should be RecoverableError
    let val = result.unwrap();
    assert!(val["error"].as_str().unwrap().contains("read_markdown"));
}
```

- [ ] **Step 2: Add `edit_file_blocks_markdown` test**

```rust
#[tokio::test]
async fn edit_file_blocks_markdown() {
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Hello\n\nworld").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = EditFile.call(json!({
        "path": "test.md",
        "old_string": "world",
        "new_string": "earth"
    }), &ctx).await;
    let val = result.unwrap();
    assert!(val["error"].as_str().unwrap().contains("edit_markdown"));
}
```

- [ ] **Step 3: Add `edit_file_allows_md_prepend_append` test**

```rust
#[tokio::test]
async fn edit_file_allows_md_prepend_append() {
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Hello\n\nworld").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = EditFile.call(json!({
        "path": "test.md",
        "new_string": "prepended\n",
        "insert": "prepend"
    }), &ctx).await;
    assert!(result.is_ok());
}
```

- [ ] **Step 4: Add `read_file_allows_md_buffer_refs` test**

```rust
#[tokio::test]
async fn read_file_allows_md_buffer_refs() {
    // Buffer refs start with "@" and should bypass the .md gate
    // This test verifies the gate check excludes buffer refs
    // (Implementation detail: buffer refs are resolved before extension check)
}
```

- [ ] **Step 5: Add `read_markdown_rejects_non_md` test**

In `src/tools/markdown.rs` tests:
```rust
#[tokio::test]
async fn read_markdown_rejects_non_md() {
    let dir = tempdir().unwrap();
    let rs = dir.path().join("main.rs");
    std::fs::write(&rs, "fn main() {}").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = ReadMarkdown.call(json!({"path": "main.rs"}), &ctx).await;
    let val = result.unwrap();
    assert!(val["error"].as_str().unwrap().contains("read_file"));
}
```

- [ ] **Step 6: Add `edit_markdown_action_edit` test**

```rust
#[tokio::test]
async fn edit_markdown_action_edit() {
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Section\n\nold text here\n").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = EditMarkdown.call(json!({
        "path": "test.md",
        "heading": "# Section",
        "action": "edit",
        "old_string": "old text",
        "new_string": "new text"
    }), &ctx).await;
    assert!(result.is_ok());
    let content = std::fs::read_to_string(&md).unwrap();
    assert!(content.contains("new text here"));
    assert!(!content.contains("old text here"));
}
```

- [ ] **Step 7: Add `edit_markdown_batch` test**

```rust
#[tokio::test]
async fn edit_markdown_batch() {
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# A\n\nalpha\n\n# B\n\nbeta\n").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = EditMarkdown.call(json!({
        "path": "test.md",
        "edits": [
            {"heading": "# A", "action": "replace", "content": "replaced alpha"},
            {"heading": "# B", "action": "edit", "old_string": "beta", "new_string": "gamma"}
        ]
    }), &ctx).await;
    assert!(result.is_ok());
    let content = std::fs::read_to_string(&md).unwrap();
    assert!(content.contains("replaced alpha"));
    assert!(content.contains("gamma"));
    assert!(!content.contains("beta"));
}
```

- [ ] **Step 8: Add `edit_markdown_rejects_non_md` test**

```rust
#[tokio::test]
async fn edit_markdown_rejects_non_md() {
    let dir = tempdir().unwrap();
    let rs = dir.path().join("main.rs");
    std::fs::write(&rs, "fn main() {}").unwrap();
    let ctx = make_ctx(dir.path()).await;
    let result = EditMarkdown.call(json!({
        "path": "main.rs",
        "heading": "# A",
        "action": "replace",
        "content": "new"
    }), &ctx).await;
    let val = result.unwrap();
    assert!(val["error"].as_str().unwrap().contains("edit_file"));
}
```

- [ ] **Step 9: Run all tests**

Run: `cargo test 2>&1`
Expected: All pass.

- [ ] **Step 10: Commit**

```bash
git add src/tools/file.rs src/tools/markdown.rs
git commit -m "test: new gate tests and markdown tool tests"
```

---

### Task 10: Update prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/tools/workflow.rs`

These are text-heavy changes. Every reference to old tool names and parameter names must be updated.

- [ ] **Step 1: Update `src/prompts/server_instructions.md`**

Global replacements across the file:
- `search_pattern` → `grep` (in tool references, examples, tables)
- `find_file` → `glob` (in tool references, examples, tables)
- `edit_section` → `edit_markdown` (in tool references, examples, tables)
- `name_path` → `symbol` (in parameter references)
- `find_symbol(pattern)` → `find_symbol(query)` (in tool signatures)
- `find_references(name_path, path)` → `find_references(symbol, path)`
- `replace_symbol(name_path, path, new_body)` → `replace_symbol(symbol, path, new_body)`
- `insert_code(name_path, path, code, position)` → `insert_code(symbol, path, code, position)`
- `remove_symbol(name_path, path)` → `remove_symbol(symbol, path)`
- `rename_symbol(name_path, path, new_name)` → `rename_symbol(symbol, path, new_name)`

**Add `read_markdown` to the Tool Reference, File I/O section:**
```markdown
- `read_markdown(path)` — read markdown files with heading navigation. Returns heading
  map by default. Use `heading=` or `headings=[]` for targeted sections, or
  `start_line`/`end_line` for line ranges.
```

**Update `edit_markdown` entry (was `edit_section`):**
```markdown
- `edit_markdown(path, heading, action, content?)` — markdown editing: section ops
  (replace, insert_before, insert_after, remove) and scoped string replacement
  (action="edit" with old_string/new_string). Batch mode via `edits=[]` array.
  `heading` uses fuzzy matching.
```

**Update Anti-Patterns table — add markdown redirect rows:**
```markdown
| `read_file` on a `.md` file | `read_markdown(path)` | Heading navigation > line guessing |
| `edit_file` to change a markdown section | `edit_markdown(path, heading, action, content)` | Heading-addressed > string matching |
```

**Update the "How to Choose" table:**
```markdown
| **A text pattern** (regex, error message) | `grep(pattern)` | `find_symbol` on matched files |
| **A filename** (glob pattern) | `glob(pattern)` | `read_file` or `list_symbols` on result |
```

**Update the Editing a Markdown Document workflow:**
```markdown
| 1 | `read_markdown(path)` | Get heading map |
| 2 | `read_markdown(path, headings=[...])` | Read target sections |
| 3a | `edit_markdown(path, heading, action, content)` | Whole-section: replace, insert, remove |
| 3b | `edit_markdown(path, heading, action="edit", old_string, new_string)` | Surgical: scoped string replacement |
| 3c | `edit_markdown(path, edits=[...])` | Batch: multiple edits, atomic |
```

**Update the Rules section if it references old names.**

- [ ] **Step 2: Update `src/prompts/onboarding_prompt.md`**

Apply the same renames:
- `search_pattern` → `grep` (lines ~106, ~115, ~204, ~230, ~267, ~321, ~581)
- `edit_section` → `edit_markdown` (line ~550)
- `find_file` → `glob` if referenced

Update the markdown editing guidance (line ~550):
```markdown
// OLD:
**Editing markdown files later:** Use `edit_section` to replace/insert/remove sections
// NEW:
**Editing markdown files later:** Use `edit_markdown` to replace/insert/remove sections, or `read_markdown` for heading-based reading
```

- [ ] **Step 3: Update `build_system_prompt_draft()` in workflow.rs**

**Lines ~393-423:** Change all `name_path` references to `symbol`:
```rust
// OLD:
"- name_path: `StructName/method`, `impl Trait for Type/method`\n\"
// NEW:
"- symbol: `StructName/method`, `impl Trait for Type/method`\n\"
```

Apply to all language-specific hints (Rust, Python, TypeScript, Go, Java, C++).

**Line ~401:**
```rust
// OLD:
"- Decorators aren't in name_path — search for the function name\n\"
// NEW:
"- Decorators aren't in symbol — search for the function name\n\"
```

**Line ~419:**
```rust
// OLD:
"- Annotations aren't in name_path — search by method name\n\"
// NEW:
"- Annotations aren't in symbol — search by method name\n\"
```

**Line ~813:**
```rust
// OLD:
"Use `scope=\"lib:<name>\"` with `find_symbol`, `list_symbols`, `search_pattern`, ..."
// NEW:
"Use `scope=\"lib:<name>\"` with `find_symbol`, `list_symbols`, `grep`, ..."
```

**Update test (line ~3878-3879):**
```rust
// OLD:
draft.contains("name_path"),
"hints should mention name_path"
// NEW:
draft.contains("symbol"),
"hints should mention symbol"
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build 2>&1`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/tools/workflow.rs
git commit -m "docs: update all three prompt surfaces for naming redesign"
```

---

### Task 11: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Clippy**

Run: `cargo clippy -- -D warnings 2>&1`
Expected: No warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Grep for leftover old names**

Run: `grep -rn "search_pattern\|find_file\|edit_section\|name_path\|max_results" src/tools/ src/server.rs src/util/path_security.rs src/prompts/ --include="*.rs" --include="*.md" | grep -v "// " | grep -v "test" | grep -v ".name_path"`

Check for any missed references. Internal Rust field names like `sym.name_path` are expected and fine — only JSON-facing strings and user-visible text should be updated.

- [ ] **Step 5: Build release binary**

Run: `cargo build --release 2>&1`
Expected: Builds successfully.

- [ ] **Step 6: Squash into clean commits if needed**

If any fixup commits were made during development, squash them into the logical commits:
1. Parameter renames (Tasks 1-3)
2. Tool renames (Task 4)
3. Markdown tools (Tasks 5-6)
4. Server wiring + gates (Tasks 7-8)
5. New tests (Task 9)
6. Prompt surfaces (Task 10)

- [ ] **Step 7: Final commit message**

If squashing everything into one commit:
```bash
git rebase -i HEAD~N  # squash to logical commits
```

Or keep the granular commits — they follow the project's commit discipline.
