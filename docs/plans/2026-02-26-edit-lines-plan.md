# `edit_lines` Tool Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a line-based splice editing tool that uses positional addressing instead of find-and-replace, eliminating token waste and match ambiguity.

**Architecture:** New `EditLines` struct implementing the `Tool` trait in `src/tools/file.rs`, registered in `src/server.rs`, with server instructions updated. Uses the same `validate_write_path` security as `ReplaceContent`.

**Tech Stack:** Rust, serde_json, anyhow. No new dependencies.

---

### Task 1: Write failing tests for `EditLines`

**Files:**
- Modify: `src/tools/file.rs` (tests module, after `replace_content_*` tests around line 988)

**Step 1: Add test functions**

Add these tests inside the existing `mod tests` block in `src/tools/file.rs`. They follow the exact pattern of the existing `replace_content_*` tests — use `project_ctx()`, write a temp file, call the tool, assert result + file content.

```rust
    #[tokio::test]
    async fn edit_lines_replace_single_line() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "delete_count": 1,
                    "new_text": "replaced"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["lines_deleted"], 1);
        assert_eq!(result["lines_inserted"], 1);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "line1\nreplaced\nline3\n");
    }

    #[tokio::test]
    async fn edit_lines_replace_multiple_lines() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "a\nb\nc\nd\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "delete_count": 2,
                    "new_text": "X\nY\nZ"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["lines_deleted"], 2);
        assert_eq!(result["lines_inserted"], 3);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "a\nX\nY\nZ\nd\n");
    }

    #[tokio::test]
    async fn edit_lines_insert_before_line() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "delete_count": 0,
                    "new_text": "inserted"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["lines_deleted"], 0);
        assert_eq!(result["lines_inserted"], 1);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "line1\ninserted\nline2\n");
    }

    #[tokio::test]
    async fn edit_lines_append_at_end() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 3,
                    "delete_count": 0,
                    "new_text": "line3"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["lines_deleted"], 0);
        assert_eq!(result["lines_inserted"], 1);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "line1\nline2\nline3\n");
    }

    #[tokio::test]
    async fn edit_lines_delete_lines() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "a\nb\nc\nd\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "delete_count": 2
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["lines_deleted"], 2);
        assert_eq!(result["lines_inserted"], 0);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "a\nd\n");
    }

    #[tokio::test]
    async fn edit_lines_start_beyond_eof_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 99,
                    "delete_count": 0,
                    "new_text": "nope"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn edit_lines_delete_past_eof_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\n").unwrap();

        let result = EditLines
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "delete_count": 5
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn edit_lines_missing_params_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "content\n").unwrap();

        let result = EditLines
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await;

        assert!(result.is_err());
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test edit_lines -- --nocapture 2>&1 | head -30`
Expected: compilation error — `EditLines` struct does not exist yet.

**Step 3: Commit**

```
git add src/tools/file.rs
git commit -m "test(tools): add failing tests for edit_lines tool"
```

---

### Task 2: Implement `EditLines` struct

**Files:**
- Modify: `src/tools/file.rs` (add struct + `impl Tool` after `ReplaceContent` impl, around line 432)

**Step 1: Add the `EditLines` struct and `Tool` impl**

Insert after the closing `}` of `impl Tool for ReplaceContent` (line 432) and before `#[cfg(test)]`:

```rust
// ── edit_lines ──────────────────────────────────────────────────────────────

pub struct EditLines;

impl Tool for EditLines {
    fn name(&self) -> &str {
        "edit_lines"
    }

    fn description(&self) -> &str {
        "Line-based splice edit. Replace, insert, or delete lines by position — no need to send old content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "start_line", "delete_count"],
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "start_line": { "type": "integer", "description": "1-based line where edit begins" },
                "delete_count": { "type": "integer", "description": "Lines to remove (0 = pure insertion)" },
                "new_text": { "type": "string", "description": "Text to insert (may contain newlines). Omit for pure deletion." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;
        let start_line = input["start_line"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("missing 'start_line' parameter"))? as usize;
        let delete_count = input["delete_count"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("missing 'delete_count' parameter"))? as usize;
        let new_text = input["new_text"].as_str().unwrap_or("");

        if start_line == 0 {
            anyhow::bail!("start_line must be >= 1 (1-based)");
        }

        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;

        let content = std::fs::read_to_string(&resolved)?;
        let had_trailing_newline = content.ends_with('\n');
        let mut lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        // start_line is 1-based; convert to 0-based index
        let idx = start_line - 1;

        // Allow idx == total for appending at end
        if idx > total {
            anyhow::bail!(
                "start_line {} is beyond end of file ({} lines)",
                start_line,
                total
            );
        }

        if idx + delete_count > total {
            anyhow::bail!(
                "cannot delete {} lines starting at line {} (file has {} lines)",
                delete_count,
                start_line,
                total
            );
        }

        // Build new text lines
        let insert_lines: Vec<&str> = if new_text.is_empty() {
            vec![]
        } else {
            new_text.lines().collect()
        };

        let lines_inserted = insert_lines.len();

        // Splice: remove delete_count lines at idx, insert new lines
        let tail: Vec<&str> = lines.split_off(idx + delete_count);
        lines.truncate(idx);
        lines.extend(insert_lines);
        lines.extend(tail);

        // Write back
        let mut out = lines.join("\n");
        if had_trailing_newline || (!content.is_empty() && lines.is_empty()) {
            // Preserve trailing newline, or keep file non-empty if we deleted everything
        }
        if had_trailing_newline && !out.is_empty() {
            out.push('\n');
        }
        std::fs::write(&resolved, &out)?;

        Ok(json!({
            "status": "ok",
            "path": resolved.display().to_string(),
            "lines_deleted": delete_count,
            "lines_inserted": lines_inserted,
            "new_total_lines": lines.len()
        }))
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test edit_lines -- --nocapture`
Expected: all 8 `edit_lines_*` tests pass.

**Step 3: Run full test suite**

Run: `cargo test`
Expected: all existing tests still pass.

**Step 4: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: clean.

**Step 5: Commit**

```
git add src/tools/file.rs
git commit -m "feat(tools): add edit_lines tool — line-based splice editing"
```

---

### Task 3: Register `EditLines` in the server

**Files:**
- Modify: `src/server.rs` — add to tool vec in `from_parts` and update test

**Step 1: Add `EditLines` to the tool registration**

In `src/server.rs` `from_parts` method, add `Arc::new(EditLines)` after `Arc::new(ReplaceContent)` in the "File tools" section. Also add the import if needed — `EditLines` is in `crate::tools::file`.

Check how `ReplaceContent` is imported — follow the same pattern. Likely via a `use crate::tools::file::*` or explicit import at the top of `server.rs`.

**Step 2: Update the `server_registers_all_tools` test**

In the `expected_tools` array in the test, add `"edit_lines"` after `"replace_content"`.

**Step 3: Run tests**

Run: `cargo test server_registers_all_tools`
Expected: pass.

Run: `cargo test`
Expected: all pass.

**Step 4: Commit**

```
git add src/server.rs
git commit -m "feat(server): register edit_lines tool"
```

---

### Task 4: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md`

**Step 1: Add `edit_lines` to the Editing section**

In the "### Editing" section (around line 63), add after `replace_content`:

```markdown
- `edit_lines(path, start_line, delete_count, [new_text])` — line-based splice edit. Preferred over `replace_content` when you know the line numbers.
```

**Step 2: Add tool selection rule**

After rule 6 (around line 91), add:

```markdown
7. **For edits to code files, prefer symbol tools** (`replace_symbol_body`, `insert_before_symbol`) over `edit_lines` or `replace_content`. Use `edit_lines` for non-code files or intra-symbol edits where you already know the line numbers.
```

**Step 3: Run build to ensure the instructions compile into the binary**

Run: `cargo build`
Expected: clean build (instructions are loaded at runtime from the file, but verify no issues).

**Step 4: Commit**

```
git add src/prompts/server_instructions.md
git commit -m "docs(prompts): add edit_lines to server instructions with tool selection guidance"
```

---

### Task 5: Final verification

**Step 1: Run full suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean, all tests pass.

**Step 2: Verify tool count**

The `server_registers_all_tools` test should now expect 31 tools (was 30).

**Step 3: Squash or leave commits as-is based on preference**

The 4 commits are:
1. `test(tools): add failing tests for edit_lines tool`
2. `feat(tools): add edit_lines tool — line-based splice editing`
3. `feat(server): register edit_lines tool`
4. `docs(prompts): add edit_lines to server instructions with tool selection guidance`
