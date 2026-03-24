# Auto-Chunked Reads Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When content exceeds the inline token budget, return as much as fits (line-aligned) instead of returning zero content, with a `complete` flag and exact continuation command.

**Architecture:** Add a `extract_lines_to_budget` helper in `src/util/text.rs` that walks lines accumulating bytes, stops at 90% of `TOOL_OUTPUT_BUFFER_THRESHOLD` (leaving headroom for JSON envelope: `content`, `complete`, `next`, `shown_lines` keys + values ≈ 500-1000 bytes), and returns the content + how many lines fit. All `read_file` overflow paths use this helper instead of returning empty responses. Every response that could be partial includes `complete: bool` and optionally `next: string`.

**Tech Stack:** Rust, serde_json, existing `OutputBuffer` infrastructure.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/util/text.rs` | Create function | `extract_lines_to_budget` — line-aligned extraction up to a byte budget |
| `src/tools/mod.rs` | Add constant | `INLINE_BYTE_BUDGET` — 90% of `TOOL_OUTPUT_BUFFER_THRESHOLD` |
| `src/tools/file.rs` | Modify | Three overflow paths in `ReadFile::call`: buffer-ref ranged, real-file ranged, buffer-ref full |
| `src/tools/file.rs` (tests) | Modify | Update existing tests, add new tests for auto-chunking behavior |
| `src/tools/file.rs` (format) | Modify | Update `format_read_file` to render `complete`/`next` fields |

**Not in scope (this plan):**
- `run_command` output — it has its own summarization pipeline (test results, build errors) that is genuinely more useful than raw first-N-lines. That's a separate follow-up.
- Full-file reads without a range — these return structured summaries (heading trees, symbol lists) which are high-value navigation aids. Keep as-is, but add `complete: false` + `file_id` (already done).
- Navigation params (`heading`, `json_path`, `toml_key`) — these already extract targeted sections and rarely overflow.

**Key design decision — line number space:**
When a ranged read (e.g. lines 50-300) overflows and gets stored as `@file_xxx`, the `next` command must use **sub-buffer-relative** line numbers (starting from 1), because that's what the `@file_xxx` handle contains. The `shown_lines` field reports the **original file line numbers** so the agent knows where it is in the real file.

---

## Chunk 1: Core Helper + Constants

### Task 1: Add `INLINE_BYTE_BUDGET` constant and `extract_lines_to_budget` helper

**Files:**
- Modify: `src/tools/mod.rs:38-42` (near existing constants)
- Modify: `src/util/text.rs` (add function + tests)

- [ ] **Step 1: Add the constant to `src/tools/mod.rs`**

Add after `TOOL_OUTPUT_BUFFER_THRESHOLD`:

```rust
/// Byte budget for auto-chunked inline content. Set to 90% of
/// TOOL_OUTPUT_BUFFER_THRESHOLD to leave headroom for the JSON envelope
/// overhead (~500-1000 bytes for content/complete/next/shown_lines keys).
pub(crate) const INLINE_BYTE_BUDGET: usize = TOOL_OUTPUT_BUFFER_THRESHOLD * 9 / 10;
```

- [ ] **Step 2: Write the failing tests in `src/util/text.rs`**

Add to the existing `tests` module:

```rust
#[test]
fn extract_lines_to_budget_fits_all() {
    let text = "short\nlines\nhere\n";
    let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 10_000);
    assert_eq!(lines_shown, 3);
    assert!(complete);
    assert_eq!(content, "short\nlines\nhere");
}

#[test]
fn extract_lines_to_budget_truncates_at_budget() {
    // Each line is 10 bytes ("line NNNN\n"). Budget of 25 bytes fits 2 full lines.
    let text: String = (1..=10).map(|i| format!("line {:04}\n", i)).collect();
    let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
    assert_eq!(lines_shown, 2);
    assert!(!complete);
    assert_eq!(content, "line 0001\nline 0002");
}

#[test]
fn extract_lines_to_budget_respects_start_line() {
    let text = "aaa\nbbb\nccc\nddd\neee\n";
    let (content, lines_shown, complete) = extract_lines_to_budget(text, 3, 100, 10_000);
    assert_eq!(lines_shown, 3); // lines 3, 4, 5
    assert!(complete);
    assert_eq!(content, "ccc\nddd\neee");
}

#[test]
fn extract_lines_to_budget_respects_end_line() {
    let text = "aaa\nbbb\nccc\nddd\neee\n";
    let (content, lines_shown, complete) = extract_lines_to_budget(text, 2, 4, 10_000);
    assert_eq!(lines_shown, 3); // lines 2, 3, 4
    assert!(complete); // all requested lines fit
    assert_eq!(content, "bbb\nccc\nddd");
}

#[test]
fn extract_lines_to_budget_budget_hit_before_end_line() {
    // Request lines 1-100 but budget only fits ~2 lines
    let text: String = (1..=100).map(|i| format!("line {:04}\n", i)).collect();
    let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
    assert_eq!(lines_shown, 2);
    assert!(!complete);
    assert_eq!(content, "line 0001\nline 0002");
}

#[test]
fn extract_lines_to_budget_zero_budget_returns_nothing() {
    let text = "aaa\nbbb\n";
    let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 0);
    assert_eq!(lines_shown, 0);
    assert!(!complete);
    assert_eq!(content, "");
}

#[test]
fn extract_lines_to_budget_single_line_exceeds_budget() {
    // A single very long line — must still return at least 1 line if budget > 0
    // to avoid infinite loops (agent would retry same range forever).
    let text = "a".repeat(1000);
    let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 1, 50);
    assert_eq!(lines_shown, 1);
    // complete = true because we reached end_line, even though it exceeded budget
    assert!(complete);
    assert_eq!(content.len(), 1000);
}

#[test]
fn extract_lines_to_budget_empty_text() {
    let (content, lines_shown, complete) = extract_lines_to_budget("", 1, 100, 10_000);
    assert_eq!(lines_shown, 0);
    assert!(complete); // no lines to show, so "all" lines were shown
    assert_eq!(content, "");
}

#[test]
fn extract_lines_to_budget_start_beyond_total() {
    let text = "aaa\nbbb\nccc\n";
    let (content, lines_shown, complete) = extract_lines_to_budget(text, 500, 600, 10_000);
    assert_eq!(lines_shown, 0);
    assert!(complete); // no lines in range, nothing to show
    assert_eq!(content, "");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test extract_lines_to_budget -- --test-threads=1`
Expected: FAIL — function doesn't exist yet

- [ ] **Step 4: Implement `extract_lines_to_budget`**

Add to `src/util/text.rs`:

```rust
/// Extract lines from `start_line` to `end_line` (1-indexed, inclusive) without
/// exceeding `byte_budget` bytes. Returns `(content, lines_shown, complete)`.
///
/// - `content`: the extracted lines joined with `\n`
/// - `lines_shown`: number of lines included
/// - `complete`: true if all lines in the requested range were included
///
/// **Safety valve:** always includes at least 1 line (even if it exceeds the budget)
/// to prevent infinite retry loops where the agent keeps requesting the same range.
pub fn extract_lines_to_budget(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
) -> (String, usize, bool) {
    let mut result_lines: Vec<&str> = Vec::new();
    let mut bytes_used: usize = 0;
    let mut hit_end = true; // assume complete unless budget breaks us out

    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if lineno < start_line {
            continue;
        }
        if lineno > end_line {
            break;
        }

        let line_bytes = line.len() + 1; // +1 for the \n join separator
        if bytes_used + line_bytes > byte_budget && !result_lines.is_empty() {
            hit_end = false;
            break;
        }

        result_lines.push(line);
        bytes_used += line_bytes;
    }

    // complete = we exited the loop naturally (hit end_line or ran out of lines),
    // not because the budget was exhausted
    let lines_shown = result_lines.len();
    (result_lines.join("\n"), lines_shown, hit_end)
}
```

Key design points:
- Single pass — no double iteration. `complete` is determined by whether the loop exited naturally (`hit_end = true`) or broke due to budget.
- Safety valve: first line always included even if it exceeds budget (prevents infinite retry).
- `complete = true` for empty ranges (start beyond total, empty text) — there is nothing left to show.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test extract_lines_to_budget`
Expected: all 9 pass

- [ ] **Step 6: Run full check**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: clean

- [ ] **Step 7: Commit**

```bash
git add src/util/text.rs src/tools/mod.rs
git commit -m "feat(read_file): add extract_lines_to_budget helper + INLINE_BYTE_BUDGET constant

Foundation for auto-chunked reads: line-aligned extraction that respects
a byte budget, always returns at least 1 line to prevent retry loops.
Single-pass algorithm — complete flag determined from loop state."
```

---

## Chunk 2: Auto-Chunking in All Three `read_file` Overflow Paths

### Task 2: Update buffer-ref ranged read to auto-chunk

**Files:**
- Modify: `src/tools/file.rs:160-175` (the `if crate::tools::exceeds_inline_limit(&content)` block in the buffer-ref path)

- [ ] **Step 1: Write the failing test**

Add to `src/tools/file.rs` tests:

```rust
#[tokio::test]
async fn read_file_buffer_ref_range_auto_chunks() {
    // When a buffer-ref range exceeds inline limit, the response should
    // include the first chunk of content (not zero content), plus
    // complete=false and a next command.
    let (_dir, ctx) = project_ctx().await;

    // Store ~15KB content (exceeds MAX_INLINE_TOKENS * 4 = 10KB)
    let content: String = (1..=300).map(|i| format!("line {:04} padding text here\n", i)).collect();
    let buf_id = ctx.output_buffer.store_file("test.txt".into(), content);

    let result = ReadFile
        .call(
            serde_json::json!({ "path": buf_id, "start_line": 1, "end_line": 300 }),
            &ctx,
        )
        .await
        .unwrap();

    // Must have content (not empty)
    assert!(result.get("content").is_some(), "should auto-chunk content; got: {result}");
    // Must signal incomplete
    assert_eq!(result["complete"], false, "should be incomplete; got: {result}");
    // Must have a next command
    let next = result["next"].as_str().expect("should have next command");
    // next should reference the file_id and use sub-buffer-relative line numbers
    assert!(next.contains("start_line="), "next should include start_line; got: {next}");
    let file_id = result["file_id"].as_str().expect("should have file_id");
    assert!(next.contains(file_id), "next should reference file_id; got: {next}");
    // shown_lines should report original file line numbers
    let shown = result["shown_lines"].as_array().unwrap();
    assert_eq!(shown[0], 1, "shown_lines should start at original start_line");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test read_file_buffer_ref_range_auto_chunks`
Expected: FAIL — response has no `content` field

- [ ] **Step 3: Rewrite the buffer-ref ranged overflow block**

Replace the existing block at `src/tools/file.rs:160-175` (the `if crate::tools::exceeds_inline_limit(&content)` inside the buffer-ref ranged path). The extracted `content` is already the sub-range, so `extract_lines_to_budget` starts at line 1 of that sub-content. Line numbers in `shown_lines` map back to the original file, but `next` uses sub-buffer line numbers since `file_id` points to the sub-buffer:

```rust
if crate::tools::exceeds_inline_limit(&content) {
    let content_total = content.lines().count();
    let file_id = ctx
        .output_buffer
        .store_file(format!("{}[{}-{}]", path, s, e), content.clone());

    // Auto-chunk: extract as much as fits the inline budget.
    // content is already the sub-range, so we work in sub-buffer line space (1-based).
    let (chunk, lines_shown, complete) =
        crate::util::text::extract_lines_to_budget(
            &content, 1, usize::MAX, crate::tools::INLINE_BYTE_BUDGET,
        );
    // shown_lines reports original file line numbers for agent context
    let orig_start = s as usize;
    let orig_end = orig_start + lines_shown - 1;
    let mut result = json!({
        "content": chunk,
        "file_id": file_id,
        "total_lines": content_total,
        "shown_lines": [orig_start, orig_end],
        "complete": complete,
    });
    if !complete {
        // next uses sub-buffer line numbers (file_id contains the sub-range)
        let buf_next_start = lines_shown + 1;
        let buf_next_end = (buf_next_start + lines_shown - 1).min(content_total);
        result["next"] = json!(format!(
            "read_file(\"{file_id}\", start_line={buf_next_start}, end_line={buf_next_end})"
        ));
    }
    return Ok(result);
}
```

- [ ] **Step 4: Run test**

Run: `cargo test read_file_buffer_ref_range_auto_chunks`
Expected: PASS

---

### Task 3: Update real-file ranged read to auto-chunk

**Files:**
- Modify: `src/tools/file.rs:391-415` (the `if crate::tools::exceeds_inline_limit(&content)` block in the real-file explicit-range path)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn read_file_real_file_range_auto_chunks() {
    let (dir, ctx) = project_ctx().await;

    // Create a file > 10KB
    let content: String = (1..=300).map(|i| format!("line {:04} padding text here\n", i)).collect();
    std::fs::write(dir.path().join("big.txt"), &content).unwrap();

    let result = ReadFile
        .call(
            serde_json::json!({ "path": "big.txt", "start_line": 1, "end_line": 300 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.get("content").is_some(), "should auto-chunk; got: {result}");
    assert_eq!(result["complete"], false);
    let next = result["next"].as_str().expect("should have next");
    assert!(result["file_id"].as_str().is_some());
    // next uses sub-buffer line numbers
    assert!(next.contains("start_line="), "next should include continuation; got: {next}");
    // shown_lines reports original file line numbers
    let shown = result["shown_lines"].as_array().unwrap();
    assert_eq!(shown[0], 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test read_file_real_file_range_auto_chunks`
Expected: FAIL

- [ ] **Step 3: Rewrite the real-file ranged overflow block**

Same pattern as Task 2 but in the real-file path. Key difference: uses `resolved.to_string_lossy()` for `store_file`, includes `source_tag`, and uses `start`/`end` instead of `s`/`e`:

```rust
if crate::tools::exceeds_inline_limit(&content) {
    let content_total = content.lines().count();
    let file_id = ctx
        .output_buffer
        .store_file(resolved.to_string_lossy().to_string(), content.clone());

    let (chunk, lines_shown, complete) =
        crate::util::text::extract_lines_to_budget(
            &content, 1, usize::MAX, crate::tools::INLINE_BYTE_BUDGET,
        );
    let orig_start = start as usize;
    let orig_end = orig_start + lines_shown - 1;
    let mut result = json!({
        "content": chunk,
        "file_id": file_id,
        "total_lines": content_total,
        "shown_lines": [orig_start, orig_end],
        "complete": complete,
    });
    if !complete {
        let buf_next_start = lines_shown + 1;
        let buf_next_end = (buf_next_start + lines_shown - 1).min(content_total);
        result["next"] = json!(format!(
            "read_file(\"{file_id}\", start_line={buf_next_start}, end_line={buf_next_end})"
        ));
    }
    if source_tag != "project" {
        result["source"] = json!(source_tag);
    }
    return Ok(result);
}
```

- [ ] **Step 4: Run test**

Run: `cargo test read_file_real_file_range_auto_chunks`
Expected: PASS

---

### Task 4: Update full-buffer-content read to auto-chunk

**Files:**
- Modify: `src/tools/file.rs:180-195` (the `if crate::tools::exceeds_inline_limit(&text)` block for full buffer reads)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn read_file_full_buffer_auto_chunks() {
    let (_dir, ctx) = project_ctx().await;

    let content: String = (1..=300).map(|i| format!("line {:04} padding text here\n", i)).collect();
    let buf_id = ctx.output_buffer.store_file("test.txt".into(), content);

    // No start_line/end_line — full buffer read
    let result = ReadFile
        .call(serde_json::json!({ "path": &buf_id }), &ctx)
        .await
        .unwrap();

    assert!(result.get("content").is_some(), "should auto-chunk; got: {result}");
    assert_eq!(result["complete"], false);
    // next should reference the SAME buffer ID (not re-buffer)
    let next = result["next"].as_str().unwrap();
    assert!(next.contains(&buf_id), "next should reference original buffer; got: {next}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test read_file_full_buffer_auto_chunks`
Expected: FAIL — current code returns only `total_lines` + `hint`, no content

- [ ] **Step 3: Rewrite the full-buffer overflow block**

Replace the existing block at `src/tools/file.rs:180-195`. This path deliberately does NOT re-buffer (avoids circular buffering). The `next` command references the original `@file_*`/`@cmd_*` handle directly:

```rust
if crate::tools::exceeds_inline_limit(&text) {
    let (chunk, lines_shown, complete) =
        crate::util::text::extract_lines_to_budget(
            &text, 1, usize::MAX, crate::tools::INLINE_BYTE_BUDGET,
        );
    let mut result = json!({
        "content": chunk,
        "total_lines": total_lines,
        "shown_lines": [1, lines_shown],
        "complete": complete,
    });
    if !complete {
        let next_start = lines_shown + 1;
        let next_end = (next_start + lines_shown - 1).min(total_lines);
        // Reference the SAME buffer path — do not re-buffer
        result["next"] = json!(format!(
            "read_file(\"{path}\", start_line={next_start}, end_line={next_end})"
        ));
    }
    return Ok(result);
}
```

- [ ] **Step 4: Update `read_file_buffered_range_shows_hint` test**

The existing test checks for the old response shape (`file_id` + `total_lines` + `hint`, no content). Replace it entirely:

```rust
#[test]
fn read_file_buffered_range_shows_hint() {
    // Auto-chunked response: has content + shown_lines + complete + next
    let val = serde_json::json!({
        "content": "line 0001 padding text\nline 0002 padding text\nline 0003 padding text",
        "file_id": "@file_abc123",
        "total_lines": 311,
        "shown_lines": [1, 3],
        "complete": false,
        "next": "read_file(\"@file_abc123\", start_line=4, end_line=6)"
    });
    let result = format_read_file(&val);
    assert!(
        result.contains("line 0001"),
        "should show content; got: {result}"
    );
    assert!(
        result.contains("3 of 311"),
        "should show progress; got: {result}"
    );
    assert!(
        result.contains("start_line=4"),
        "should show next command; got: {result}"
    );
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat(read_file): auto-chunk all three overflow paths

Instead of returning zero content when reads exceed the inline token
budget, return the first chunk that fits plus complete=false and a
next continuation command. Covers buffer-ref ranged, real-file ranged,
and full-buffer reads. Sub-buffer line numbers in next commands avoid
the line-space confusion bug."
```

---

## Chunk 3: Update `format_read_file` + Server Instructions

### Task 5: Rewrite `format_read_file` branch ordering

**Files:**
- Modify: `src/tools/file.rs:930-950` (`format_read_file` function)

The function currently has these branches:
1. Summary mode (`type` key) → `format_read_file_summary`
2. No-content buffered mode (`content` is None + `file_id` present) → compact buffer display
3. Normal content mode → line-numbered content

The new auto-chunked responses have `content` + `shown_lines` + `complete`, so they would fall through to the normal content path (branch 3) which doesn't render `complete`/`next`. We need a new branch:

1. Summary mode (`type` key) → unchanged
2. **Auto-chunked mode (`shown_lines` key)** → NEW: line-numbered content with progress + next
3. No-content buffered mode → REMOVE (all overflow paths now return content)
4. Normal content mode → unchanged

- [ ] **Step 1: Write the tests**

```rust
#[test]
fn format_read_file_auto_chunked() {
    let val = serde_json::json!({
        "content": "line 0001 text\nline 0002 text\nline 0003 text",
        "total_lines": 300,
        "shown_lines": [1, 3],
        "complete": false,
        "file_id": "@file_abc123",
        "next": "read_file(\"@file_abc123\", start_line=4, end_line=6)"
    });
    let result = format_read_file(&val);
    // Should show line-numbered content
    assert!(result.contains("line 0001"), "should show content; got: {result}");
    // Line numbers should start at shown_lines[0]
    assert!(result.contains("1|"), "should have line numbers; got: {result}");
    // Progress indicator
    assert!(result.contains("3 of 300"), "should show progress; got: {result}");
    // Next command
    assert!(result.contains("start_line=4"), "should show next; got: {result}");
    // Buffer ref
    assert!(result.contains("@file_abc123"), "should show buffer ref; got: {result}");
}

#[test]
fn format_read_file_auto_chunked_mid_file() {
    // Chunk from the middle of a file — line numbers should start at 50
    let val = serde_json::json!({
        "content": "middle content\nmore content",
        "total_lines": 300,
        "shown_lines": [50, 51],
        "complete": false,
        "next": "read_file(\"@file_abc\", start_line=52, end_line=53)"
    });
    let result = format_read_file(&val);
    assert!(result.contains("50|"), "line numbers should start at 50; got: {result}");
    assert!(result.contains("51|"), "should have line 51; got: {result}");
}

#[test]
fn format_read_file_auto_chunked_complete() {
    let val = serde_json::json!({
        "content": "line 1\nline 2",
        "total_lines": 2,
        "shown_lines": [1, 2],
        "complete": true,
    });
    let result = format_read_file(&val);
    assert!(result.contains("line 1"), "should show content; got: {result}");
    assert!(!result.contains("Next:"), "should not show next for complete reads; got: {result}");
}
```

- [ ] **Step 2: Implement the auto-chunked branch in `format_read_file`**

Insert a new branch AFTER the summary check but BEFORE the old no-content check:

```rust
fn format_read_file(val: &Value) -> String {
    // Summary modes have a "type" key
    if let Some(file_type) = val["type"].as_str() {
        return format_read_file_summary(val, file_type);
    }

    // Auto-chunked response: shown_lines present means partial read with content
    if let Some(shown) = val.get("shown_lines").and_then(|v| v.as_array()) {
        let start = shown.first().and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let end = shown.last().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let total = val["total_lines"].as_u64().unwrap_or(0);
        let complete = val["complete"].as_bool().unwrap_or(true);
        let lines_shown = end.saturating_sub(start) + 1;

        let content = val["content"].as_str().unwrap_or("");
        let lines: Vec<&str> = content.lines().collect();
        let lineno_width = end.to_string().len();

        let mut out = format!("{total} lines\n");
        for (i, line) in lines.iter().enumerate() {
            let lineno = start + i;
            out.push('\n');
            out.push_str(&format!("{:>width$}| {line}", lineno, width = lineno_width));
        }

        if let Some(file_id) = val["file_id"].as_str() {
            out.push_str(&format!("\n\n  Buffer: {file_id}"));
        }
        if !complete {
            out.push_str(&format!("\n  [{lines_shown} of {total} lines shown]"));
            if let Some(next) = val["next"].as_str() {
                out.push_str(&format!("\n  Next: {next}"));
            }
        }
        return out;
    }

    // Old no-content buffered mode (may still appear for backward compat — remove
    // once all callers produce auto-chunked responses)
    if val.get("content").is_none() {
        if let Some(file_id) = val["file_id"].as_str() {
            let total = val["total_lines"].as_u64().unwrap_or(0);
            let mut out = format!("{total} lines\n\n  Buffer: {file_id}");
            if let Some(hint) = val["hint"].as_str() {
                out.push_str(&format!("\n  {hint}"));
            }
            return out;
        }
    }

    // Content mode (unchanged from here)
    // ...existing code...
```

- [ ] **Step 3: Run all tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat(read_file): format_read_file renders auto-chunked responses

Shows line-numbered content starting at shown_lines[0], progress
indicator, buffer ref, and next continuation command. Branch ordering:
summary → auto-chunked → old-no-content (compat) → normal content."
```

---

### Task 6: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Update `read_file` documentation**

In the `read_file` tool description, add a note about auto-chunking behavior:

> Large content is auto-chunked: the response includes as much content as fits inline, plus
> `complete: false` and a `next` field with the exact continuation command to get more.
> No need to guess chunk sizes — just follow the `next` command to continue reading.

- [ ] **Step 2: Update the output buffer / access patterns section**

Add `complete` and `next` to the response field documentation. Note that `shown_lines` reports original file line numbers.

- [ ] **Step 3: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: document auto-chunked read_file behavior in server instructions"
```

---

## Verification

After all tasks:

- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` — all pass
- [ ] Manual test: `cargo build --release`, restart MCP, read a large file, verify auto-chunking works
- [ ] Verify the `next` command is copy-pasteable and produces the next chunk
- [ ] Verify `complete: true` when reading a small file or the final chunk
- [ ] Verify `shown_lines` reports original file line numbers while `next` uses sub-buffer line numbers
