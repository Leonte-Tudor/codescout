# read_file / replace_content Restrictions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove `replace_content` tool and block `read_file` for source code files, forcing symbol tool usage.

**Architecture:** Gate `read_file` via `ast::detect_language()` returning `RecoverableError` for source files. Delete `replace_content` entirely (6 locations per the new-tool checklist).

**Tech Stack:** Rust, code-explorer tool framework, `RecoverableError` pattern

**Design doc:** `docs/plans/2026-02-26-read-file-replace-content-restrictions-design.md`

---

### Task 1: Remove `replace_content` — tool registration and security gate

**Files:**
- Modify: `src/server.rs:24` (import), `src/server.rs:67` (registration)
- Modify: `src/util/path_security.rs:257` (security gate)

**Step 1: Remove import from server.rs**

In `src/server.rs:24`, change the file import line:
```rust
// Before:
    file::{
        CreateTextFile, EditLines, FindFile, ListDir, ReadFile, ReplaceContent, SearchForPattern,
    },
// After:
    file::{CreateTextFile, EditLines, FindFile, ListDir, ReadFile, SearchForPattern},
```

**Step 2: Remove registration from server.rs**

Delete line `src/server.rs:67`:
```rust
            Arc::new(ReplaceContent),
```

**Step 3: Remove from security gate**

In `src/util/path_security.rs:256-257`, remove the `"replace_content"` arm:
```rust
// Before:
        "create_text_file"
        | "replace_content"
        | "edit_lines"
// After:
        "create_text_file"
        | "edit_lines"
```

**Step 4: Run `cargo build` to verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: warnings about dead code in `ReplaceContent` (struct defined but unused). No errors.

---

### Task 2: Remove `replace_content` — struct, impl, and tests

**Files:**
- Modify: `src/tools/file.rs:378-447` (struct + impl), `src/tools/file.rs:1013-1534` (tests)

**Step 1: Delete the `ReplaceContent` struct and impl**

Delete `src/tools/file.rs:376-448` — the blank line, `pub struct ReplaceContent;`, and the
entire `impl Tool for ReplaceContent` block.

**Step 2: Delete all `replace_content` tests**

Delete these test functions from the `tests` module in `src/tools/file.rs`:
- `replace_content_literal` (lines 1013-1034)
- `replace_content_literal_first_only` (lines 1037-1057)
- `replace_content_regex` (lines 1060-1081)
- `replace_content_regex_first_only` (lines 1084-1105)
- `replace_content_no_match` (lines 1108-1127)
- `replace_content_missing_params_errors` (lines 1130-1139)
- `replace_content_missing_params_errors_security` (lines 1239-1256)
- `replace_content_invalid_regex_errors` (lines 1285-1302)
- `replace_content_nonexistent_file_errors` (lines 1315-1329)
- `replace_content_no_matches_reports_zero` (lines 1332-1352)
- `replace_content_outside_project_rejected` (lines 1436-1458)
- `replace_content_huge_regex_rejected` (lines 1516-1534)

**Step 3: Run `cargo build`**

Run: `cargo build 2>&1 | tail -5`
Expected: Clean compilation, no warnings about `ReplaceContent`.

**Step 4: Commit**

```bash
git add src/tools/file.rs src/server.rs src/util/path_security.rs
git commit -m "refactor: remove replace_content tool

Builtin Edit tool handles non-code files. Symbol tools and edit_lines
handle code files. replace_content was a bypass path that undermined
symbol-level navigation."
```

---

### Task 3: Remove `replace_content` from security tests and tool list test

**Files:**
- Modify: `src/util/path_security.rs` — security tests
- Modify: `src/server.rs:393` — `server_registers_all_tools` test

**Step 1: Update `file_write_enabled_by_default` test**

In `src/util/path_security.rs`, remove this line from the test:
```rust
        assert!(check_tool_access("replace_content", &config).is_ok());
```

**Step 2: Update `file_write_disabled_blocks_all_write_tools` test**

Remove `"replace_content",` from the tool list array in that test.

**Step 3: Update `server_registers_all_tools` test**

In `src/server.rs`, remove `"replace_content",` from the `expected_tools` array.
Also update the expected tool count if there's an assertion on it.

**Step 4: Run all tests**

Run: `cargo test 2>&1 | tail -10`
Expected: All tests pass. Test count reduced by ~12 (removed replace_content tests).

**Step 5: Commit**

```bash
git add src/util/path_security.rs src/server.rs
git commit -m "test: update tests for replace_content removal"
```

---

### Task 4: Add `read_file` source code gate — tests first

**Files:**
- Modify: `src/tools/file.rs` — add tests in `mod tests`

**Step 1: Write the failing tests**

Add these tests to the `tests` module in `src/tools/file.rs`:

```rust
    #[tokio::test]
    async fn read_file_blocks_source_code_files() {
        let (dir, ctx) = project_ctx().await;
        let rs_file = dir.path().join("main.rs");
        std::fs::write(&rs_file, "fn main() {}\n").unwrap();

        let result = ReadFile.call(json!({ "path": rs_file.to_str().unwrap() }), &ctx).await;
        // Should be a RecoverableError (propagated as Err), not Ok
        assert!(result.is_err(), "read_file should block .rs files");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("source code"),
            "error should mention source code: {err_msg}"
        );
    }

    #[tokio::test]
    async fn read_file_allows_non_source_files() {
        let (dir, ctx) = project_ctx().await;
        let toml_file = dir.path().join("config.toml");
        std::fs::write(&toml_file, "key = \"value\"\n").unwrap();

        let result = ReadFile.call(json!({ "path": toml_file.to_str().unwrap() }), &ctx).await;
        assert!(result.is_ok(), "read_file should allow .toml files");
    }

    #[tokio::test]
    async fn read_file_allows_markdown_files() {
        let (dir, ctx) = project_ctx().await;
        let md_file = dir.path().join("README.md");
        std::fs::write(&md_file, "# Hello\n").unwrap();

        let result = ReadFile.call(json!({ "path": md_file.to_str().unwrap() }), &ctx).await;
        assert!(result.is_ok(), "read_file should allow .md files");
    }

    #[tokio::test]
    async fn read_file_allows_unknown_extensions() {
        let (dir, ctx) = project_ctx().await;
        let csv_file = dir.path().join("data.csv");
        std::fs::write(&csv_file, "a,b,c\n1,2,3\n").unwrap();

        let result = ReadFile.call(json!({ "path": csv_file.to_str().unwrap() }), &ctx).await;
        assert!(result.is_ok(), "read_file should allow unknown extensions");
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test read_file_blocks_source_code 2>&1 | tail -5`
Expected: FAIL — currently `read_file` allows all files.

---

### Task 5: Implement `read_file` source code gate

**Files:**
- Modify: `src/tools/file.rs` — `ReadFile::call()` method

**Step 1: Add RecoverableError import**

At the top of `src/tools/file.rs`, add `RecoverableError` to the import:
```rust
// Before:
use super::{Tool, ToolContext};
// After:
use super::{RecoverableError, Tool, ToolContext};
```

**Step 2: Add the gate after path resolution**

In `ReadFile::call()`, after the `validate_read_path` call and before the source tag
computation, add:

```rust
        // Block source code files — force symbol tool usage
        if let Some(lang) = crate::ast::detect_language(&resolved) {
            if lang != "markdown" {
                return Err(RecoverableError::with_hint(
                    "read_file is not available for source code files",
                    "Use symbol tools instead:\n  \
                     get_symbols_overview(path) — see all symbols + line numbers\n  \
                     find_symbol(name, include_body=true) — read a specific symbol body\n  \
                     list_functions(path) — quick function signatures",
                )
                .into());
            }
        }
```

This goes right after the `validate_read_path` block (~line 50 in the call method,
after the `let resolved = ...?;` line).

**Step 3: Run the new tests**

Run: `cargo test read_file_blocks_source_code read_file_allows 2>&1 | tail -15`
Expected: All 4 new tests pass.

**Step 4: Run full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: All tests pass. Some existing `read_file` tests may need updating if they
read `.rs` files — check and fix any failures.

**Step 5: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: block read_file for source code files

Uses ast::detect_language() to identify source files. Returns
RecoverableError with hint to use symbol tools instead. Markdown
files are excluded (documentation, not navigable code)."
```

---

### Task 6: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md`

**Step 1: Update read_file description**

In the "Reading & Searching" section, change:
```markdown
- `read_file(path, [start_line], [end_line])` — read file content (use line ranges for large files)
```
to:
```markdown
- `read_file(path, [start_line], [end_line])` — read non-code files (README, configs, TOML, JSON, YAML). Blocked for source code files — use symbol tools instead.
```

**Step 2: Remove replace_content from Editing section**

Delete this line:
```markdown
- `replace_content(path, old, new)` — find-and-replace text
```

**Step 3: Update rule 2**

Change:
```markdown
2. **Use `read_file` for non-code files** (README, configs, TOML, JSON, YAML) or when you need a specific line range.
```
to:
```markdown
2. **`read_file` only works for non-code files** (README, configs, TOML, JSON, YAML). It will reject source code files — use `get_symbols_overview` + `find_symbol(include_body=true)` instead.
```

**Step 4: Update rule 7**

Change:
```markdown
7. **For edits to code files, prefer symbol tools** (`replace_symbol_body`, `insert_before_symbol`) over `edit_lines` or `replace_content`. Use `edit_lines` for non-code files or intra-symbol edits where you already know the line numbers.
```
to:
```markdown
7. **For edits to code files, prefer symbol tools** (`replace_symbol_body`, `insert_before_symbol`) over `edit_lines`. Use `edit_lines` for non-code files or intra-symbol edits where you already know the line numbers.
```

**Step 5: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: update server instructions for read_file restriction and replace_content removal"
```

---

### Task 7: Run full verification

**Step 1: Format**

Run: `cargo fmt`

**Step 2: Lint**

Run: `cargo clippy -- -D warnings 2>&1 | tail -10`
Expected: No warnings, no errors.

**Step 3: Full test suite**

Run: `cargo test 2>&1 | tail -15`
Expected: All tests pass. Tool count reduced from 31 to 30.

**Step 4: Smoke test**

Run: `cargo run -- start --project . <<< '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' 2>/dev/null | head -1 | jq '.result.tools | length'`
Expected: 30 (was 31).

**Step 5: Final commit if any formatting changes**

```bash
git add -A && git diff --cached --quiet || git commit -m "style: fmt"
```

---

### Task 8 (separate repo): Update routing plugin

**Repo:** `../claude-plugins/code-explorer-routing`

**Step 1: Remove edit-router.sh hook**

Delete `hooks/edit-router.sh`.

**Step 2: Update hooks.json**

Remove the entire `replace_content` matcher block:
```json
      {
        "matcher": "replace_content",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/edit-router.sh"
          }
        ]
      }
```

**Step 3: Update guidance.txt**

In the EDIT section, change:
```
EDIT code:
  Symbol-level (preferred) → replace_symbol_body / insert_before_symbol / insert_after_symbol
  Line-level (know lines)  → edit_lines(path, start_line, delete_count, new_text)
  Text find-replace (non-code only) → replace_content(path, old, new)
```
to:
```
EDIT code:
  Symbol-level (preferred) → replace_symbol_body / insert_before_symbol / insert_after_symbol
  Line-level (know lines)  → edit_lines(path, start_line, delete_count, new_text)
```

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: remove replace_content hook (tool removed from code-explorer)"
```
