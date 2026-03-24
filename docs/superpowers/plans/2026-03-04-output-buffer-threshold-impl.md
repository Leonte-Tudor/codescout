# Output Buffer Threshold + Compact Summary Cap — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Lower the `OutputBuffer` threshold from 10 KB to 5 KB and cap compact summaries at 2 KB (soft) / 3 KB (hard), so typical search/list tool results are always buffered and the LLM receives a structured compact summary + `@tool_ref` instead of a raw JSON wall.

**Architecture:** Two targeted changes in `src/tools/mod.rs` only: (1) add a private `truncate_compact()` helper, (2) update two constant values and wire the helper into the existing buffer path in `call_content()`. Zero per-tool changes.

**Tech Stack:** Rust, `serde_json`, existing `OutputBuffer` (`src/tools/output_buffer.rs`), `#[async_trait]`

---

### Task 1: Write failing tests for `truncate_compact`

**Files:**
- Modify: `src/tools/mod.rs` (tests module, after the existing `call_content_generic_fallback_without_format_compact` test)

**Context:** `truncate_compact(text, soft_max, hard_max)` doesn't exist yet — all these tests will fail to compile.

**Step 1: Add the 5 unit tests inside the `mod tests` block**

Append after the last existing test (around line 470, inside `mod tests { … }`):

```rust
    // ---- truncate_compact tests ----

    #[test]
    fn truncate_compact_under_soft_cap_returns_verbatim() {
        let text = "line1\nline2\nline3";
        assert_eq!(super::truncate_compact(text, 2_000, 3_000), text);
    }

    #[test]
    fn truncate_compact_exact_soft_cap_returns_verbatim() {
        // Exactly at the soft cap — no truncation
        let text = "x".repeat(2_000);
        assert_eq!(super::truncate_compact(&text, 2_000, 3_000), text);
    }

    #[test]
    fn truncate_compact_at_line_boundary() {
        // Line 1 is 1,800 bytes; line 2 is 600 bytes → total 2,401 (> soft_max=2_000)
        // Last '\n' is at byte 1,800, which is ≤ hard_max=3_000 → truncate there
        let line1 = "a".repeat(1_800);
        let line2 = "b".repeat(600);
        let text = format!("{}\n{}", line1, line2);

        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(result.starts_with(&line1), "should keep line1 intact");
        assert!(!result.contains(&line2), "should drop line2");
        assert!(result.contains("… (truncated)"), "should append truncation note");
    }

    #[test]
    fn truncate_compact_no_newlines_uses_hard_cap() {
        // Single 5,000-byte line — no '\n' → hard-cap at 3,000 bytes
        let text = "x".repeat(5_000);
        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(
            result.starts_with(&"x".repeat(3_000)),
            "should keep first 3,000 bytes"
        );
        assert!(result.ends_with("… (truncated)"), "should append note");
        // Sanity check: result is not longer than hard_max + note
        assert!(result.len() <= 3_000 + 20);
    }

    #[test]
    fn truncate_compact_preserves_text_exactly_at_hard_cap() {
        // Text is 2,500 bytes (> soft) with a single newline at position 2,400.
        // Line boundary (2,400) is between soft (2,000) and hard (3,000) — use it.
        let line1 = "a".repeat(2_400);
        let line2 = "b".repeat(99);
        let text = format!("{}\n{}", line1, line2);

        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(result.starts_with(&line1), "should keep line1");
        assert!(!result.contains(&line2), "should not include line2");
        assert!(result.contains("… (truncated)"));
    }
```

**Step 2: Run tests to confirm they fail**

```bash
cargo test truncate_compact -- --nocapture 2>&1 | head -20
```

Expected: compile error — `unresolved function truncate_compact`

---

### Task 2: Implement `truncate_compact`

**Files:**
- Modify: `src/tools/mod.rs` (add after the `guard_worktree_write` function, before the `Tool` trait)

**Step 1: Add the helper function**

Insert this block just before `pub trait Tool:` (around line 174):

```rust
/// Truncate a compact summary to fit within output size limits, preserving line structure.
///
/// - If `text.len() <= soft_max`: returned verbatim.
/// - Otherwise: find the last `\n` at or before `soft_max`. If that line boundary
///   is within `hard_max`, truncate there. If no usable line boundary exists,
///   truncate at `hard_max` bytes directly.
/// - Always appends `"\n… (truncated)"` when content is cut.
fn truncate_compact(text: &str, soft_max: usize, hard_max: usize) -> String {
    if text.len() <= soft_max {
        return text.to_string();
    }

    // Find the last newline at or before soft_max
    let search_end = soft_max.min(text.len());
    if let Some(nl_pos) = text[..search_end].rfind('\n') {
        let cut = &text[..nl_pos];
        if cut.len() <= hard_max {
            return format!("{}\n… (truncated)", cut);
        }
    }

    // No usable line boundary — hard-truncate at hard_max bytes
    let end = hard_max.min(text.len());
    format!("{}… (truncated)", &text[..end])
}
```

**Step 2: Run the truncate_compact tests**

```bash
cargo test truncate_compact -- --nocapture
```

Expected: all 5 PASS

**Step 3: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat: add truncate_compact helper for compact summary size bounding"
```

---

### Task 3: Write failing threshold tests

**Files:**
- Modify: `src/tools/mod.rs` (tests module — add after existing `call_content_*` tests)

**Context:** The new tests target the 5 KB threshold (not yet in place) and the summary cap. They'll pass incorrectly with the old 10 KB threshold, so we need to verify they initially fail against the *current* threshold. The key test is `call_content_buffers_at_5k_threshold` — with the current threshold of 10 KB, a 6 KB result is NOT buffered, so the assertion `text.contains("@tool_")` will fail.

**Step 1: Add the threshold + summary-cap tests**

Inside `mod tests`, after the existing `call_content_*` group:

```rust
    #[tokio::test]
    async fn call_content_buffers_at_5k_threshold() {
        // Build a Value whose JSON is ~6 KB — above the new 5 KB threshold
        // but below the old 10 KB threshold. With the new threshold this MUST buffer.
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..120)
            .map(|i| {
                serde_json::json!({
                    "file": format!("src/tools/file_{}.rs", i),
                    "line": i,
                    "content": format!("let x_{} = some_function_call_{};", i, i)
                })
            })
            .collect();
        let result = serde_json::json!({ "matches": items, "total": items.len() });

        // Sanity: confirm the JSON is in the 5–10 KB range
        let json_len = serde_json::to_string(&result).unwrap().len();
        assert!(json_len > 5_000, "test data must be > 5 KB, got {} bytes", json_len);
        assert!(json_len < 10_000, "test data must be < 10 KB, got {} bytes", json_len);

        let tool = EchoTool {
            result,
            user_summary: Some("120 matches".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("@tool_"),
            "6 KB output must be buffered, got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[tokio::test]
    async fn call_content_does_not_buffer_under_5k() {
        // ~2 KB result — must stay inline (no @tool_ ref)
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..30)
            .map(|i| serde_json::json!({ "file": format!("src/a_{}.rs", i), "line": i }))
            .collect();
        let result = serde_json::json!({ "matches": items });

        let json_len = serde_json::to_string(&result).unwrap().len();
        assert!(json_len < 5_000, "test data must be < 5 KB, got {} bytes", json_len);

        let tool = EchoTool {
            result,
            user_summary: Some("30 matches".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.contains("@tool_"),
            "small output must not be buffered, got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[tokio::test]
    async fn call_content_caps_compact_summary() {
        // format_compact returns a 4 KB summary — must be truncated to ≤ 3 KB (hard max)
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..200)
            .map(|i| serde_json::json!({ "idx": i, "name": "x".repeat(50) }))
            .collect();
        let result = serde_json::json!({ "items": items });

        // Summary deliberately larger than hard cap
        let big_summary = format!("{}\n", "summary line ".repeat(300)); // ~3.9 KB
        assert!(big_summary.len() > 3_000, "summary must be > hard cap for this test");

        let tool = EchoTool {
            result,
            user_summary: Some(big_summary),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(text.contains("@tool_"), "must be buffered");
        // The inline text must be bounded; +100 slack for "Full result: @tool_xxx\n"
        assert!(
            text.len() <= 3_000 + 100,
            "summary must be capped; got {} bytes",
            text.len()
        );
        assert!(
            text.contains("truncated"),
            "must include truncation note: {}",
            &text[..text.len().min(200)]
        );
    }
```

**Step 2: Run tests to confirm the new threshold tests fail**

```bash
cargo test call_content_buffers_at_5k_threshold -- --nocapture
```

Expected: FAIL — `assertion failed: text.contains("@tool_")` (because threshold is still 10 KB)

The other two new tests (`call_content_does_not_buffer_under_5k`, `call_content_caps_compact_summary`) should PASS at this point — that's expected, they're testing behavior that doesn't depend on the threshold value.

**Step 3: Commit the tests**

```bash
git add src/tools/mod.rs
git commit -m "test: add threshold + summary-cap tests for call_content (currently failing)"
```

---

### Task 4: Update constants and wire `truncate_compact` into `call_content`

**Files:**
- Modify: `src/tools/mod.rs` (constant values + `call_content` body)

**Step 1: Update the constants**

Find the existing constant (line 34):

```rust
pub(crate) const TOOL_OUTPUT_BUFFER_THRESHOLD: usize = 10_000;
```

Replace with:

```rust
pub(crate) const TOOL_OUTPUT_BUFFER_THRESHOLD: usize = 5_000;
/// Soft cap for compact summaries shown alongside `@tool_*` refs.
/// Truncation prefers whole-line boundaries. See [`truncate_compact`].
pub(crate) const COMPACT_SUMMARY_MAX_BYTES: usize = 2_000;
/// Hard cap — no summary will exceed this size regardless of line boundaries.
pub(crate) const COMPACT_SUMMARY_HARD_MAX_BYTES: usize = 3_000;
```

**Step 2: Update the buffer path in `call_content`**

Find the buffer path in `call_content` (around line 209–220):

```rust
        if json.len() > TOOL_OUTPUT_BUFFER_THRESHOLD {
            let json_len = json.len();
            let ref_id = ctx.output_buffer.store_tool(self.name(), json);
            let summary = self
                .format_compact(&val)
                .unwrap_or_else(|| format!("Result stored in {} ({} bytes)", ref_id, json_len));
            return Ok(vec![Content::text(format!(
                "{}\nFull result: {}",
                summary, ref_id
            ))]);
        }
```

Replace with:

```rust
        if json.len() > TOOL_OUTPUT_BUFFER_THRESHOLD {
            let json_len = json.len();
            let ref_id = ctx.output_buffer.store_tool(self.name(), json);
            let raw_summary = self
                .format_compact(&val)
                .unwrap_or_else(|| format!("Result stored in {} ({} bytes)", ref_id, json_len));
            let summary = truncate_compact(
                &raw_summary,
                COMPACT_SUMMARY_MAX_BYTES,
                COMPACT_SUMMARY_HARD_MAX_BYTES,
            );
            return Ok(vec![Content::text(format!(
                "{}\nFull result: {}",
                summary, ref_id
            ))]);
        }
```

**Step 3: Run all call_content tests**

```bash
cargo test call_content -- --nocapture
```

Expected: all PASS (including the previously-failing `call_content_buffers_at_5k_threshold`)

**Step 4: Run the full test suite**

```bash
cargo test
```

Expected: all existing tests pass. (The existing `call_content_buffers_large_output` test uses 500-item arrays ~25 KB — still passes trivially.)

**Step 5: Run clippy and fmt**

```bash
cargo clippy -- -D warnings
cargo fmt
```

Expected: no warnings, no formatting changes (the new code is already clean).

**Step 6: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat: lower OutputBuffer threshold to 5 KB, cap compact summaries at 2/3 KB"
```

---

### Task 5: Update the existing large-output test to also cover the old threshold range

**Files:**
- Modify: `src/tools/mod.rs` (tests module — update `call_content_buffers_large_output`)

**Context:** The existing test uses 500 items (~25 KB) which still works, but add a comment so it's clear why that count is used, and also verify the test name reflects the new threshold.

**Step 1: Update the comment in `call_content_buffers_large_output`**

Find this comment in the test (around line 360):

```rust
        // Build a Value that serializes to > 10_000 bytes
```

Change it to:

```rust
        // Build a Value that serializes to >> 5_000 bytes (well above the buffer threshold)
```

**Step 2: Run all tests one final time**

```bash
cargo test
cargo clippy -- -D warnings
```

Expected: all pass, no warnings.

**Step 3: Final commit**

```bash
git add src/tools/mod.rs
git commit -m "test: update comment to reflect 5 KB threshold in large-output test"
```

---

## Summary of Changes

| File | Change |
|---|---|
| `src/tools/mod.rs` | `TOOL_OUTPUT_BUFFER_THRESHOLD`: `10_000` → `5_000` |
| `src/tools/mod.rs` | Add `COMPACT_SUMMARY_MAX_BYTES = 2_000` |
| `src/tools/mod.rs` | Add `COMPACT_SUMMARY_HARD_MAX_BYTES = 3_000` |
| `src/tools/mod.rs` | Add `truncate_compact(text, soft_max, hard_max) -> String` |
| `src/tools/mod.rs` | Update `call_content` buffer path to use `truncate_compact` |
| `src/tools/mod.rs` | Add 8 new tests (5 unit + 3 integration-style) |

No other files change.
