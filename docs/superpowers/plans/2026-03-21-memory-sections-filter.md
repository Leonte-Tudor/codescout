# Memory Sections Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional `sections: string[]` param to `memory(action="read")` that returns only the requested `### Heading` blocks from a memory file, reducing context waste in multi-language workspaces.

**Architecture:** A pure `filter_sections` function in a new `src/memory/filter.rs` module does all the parsing and filtering work. `Memory::call`'s read arm (both private and shared branches) parses the `sections` input param and calls `filter_sections` before the existing inline-vs-buffer threshold logic. The server instructions are updated to document the new param.

**Tech Stack:** Rust, `serde_json::json!`, existing `RecoverableError` pattern, existing `Arc<output_buffer::OutputBuffer>::store_file`.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/memory/filter.rs` | **Create** | `FilterResult` struct + `filter_sections` pure function + all unit tests |
| `src/memory/mod.rs` | **Modify** | Add `pub mod filter;` |
| `src/tools/memory.rs` | **Modify** | Add `sections` to schema; call filter in read arm; integration tests |
| `src/prompts/server_instructions.md` | **Modify** | Document `sections` param on `action="read"` line |

---

### Task 1: Register `filter` module in `src/memory/mod.rs`

**Files:**
- Modify: `src/memory/mod.rs`

This must happen before writing `filter.rs` tests so `cargo test` can discover the module.

- [ ] **Step 1: Add `pub mod filter;`**

In `src/memory/mod.rs`, add after the existing `pub mod classify;` line:

```rust
pub mod filter;
```

- [ ] **Step 2: Create an empty `src/memory/filter.rs` so it compiles**

```rust
// src/memory/filter.rs
```

- [ ] **Step 3: Verify compile**

```bash
cargo build 2>&1 | grep error
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/memory/mod.rs src/memory/filter.rs
git commit -m "feat(memory): add filter module skeleton"
```

---

### Task 2: Create `src/memory/filter.rs` with failing unit tests

**Files:**
- Modify: `src/memory/filter.rs`

- [ ] **Step 1: Replace the empty file with the full stub + tests**

```rust
#[derive(Debug, PartialEq)]
pub struct FilterResult {
    /// Filtered markdown: preamble + matched section bodies.
    pub content: String,
    /// True if at least one requested section was found.
    pub matched: bool,
    /// Requested sections not found — preserves caller-supplied casing.
    pub missing: Vec<String>,
    /// All `### ` headings present in the file, normalized (trimmed), in file order.
    pub available: Vec<String>,
}

/// Filter markdown content to only the requested `### Heading` sections.
///
/// # Precondition
///
/// `sections` must be non-empty. Enforced via `debug_assert!` (fires in debug
/// builds / `cargo test`; compiled out in `--release`). The caller in
/// `Memory::call` checks `sections.is_empty()` before calling this function.
///
/// # Returns
///
/// Always returns a `FilterResult`. The caller checks `result.matched` to
/// decide whether to return content or a `RecoverableError`.
pub fn filter_sections(content: &str, sections: &[&str]) -> FilterResult {
    debug_assert!(!sections.is_empty(), "precondition: sections must be non-empty");
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Language Patterns

Intro line.

### Rust

Rust anti-patterns here.

#### Sub-heading

More Rust content.

### TypeScript

TypeScript patterns here.

### Python

Python patterns here.
";

    #[test]
    fn filter_sections_returns_matching_section() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"), "should include heading");
        assert!(r.content.contains("Rust anti-patterns here."), "should include body");
        assert!(r.content.contains("# Language Patterns"), "should include preamble");
        assert!(!r.content.contains("### TypeScript"), "should exclude TypeScript");
    }

    #[test]
    fn filter_sections_case_insensitive() {
        let r = filter_sections(SAMPLE, &["rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
    }

    #[test]
    fn filter_sections_multiple_sections() {
        let r = filter_sections(SAMPLE, &["Rust", "TypeScript"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        assert!(!r.content.contains("### Python"));
        assert!(r.missing.is_empty());
    }

    #[test]
    fn filter_sections_preserves_preamble() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.content.starts_with("# Language Patterns"));
    }

    #[test]
    fn filter_sections_no_match_returns_not_matched() {
        let r = filter_sections(SAMPLE, &["Go"]);
        assert!(!r.matched);
        assert_eq!(r.missing, vec!["Go"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }

    #[test]
    fn filter_sections_partial_match_returns_missing() {
        // "typescript" matches (case-insensitive); "Go" does not
        let r = filter_sections(SAMPLE, &["Rust", "typescript", "Go"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        // missing preserves caller-supplied casing
        assert_eq!(r.missing, vec!["Go"]);
    }

    #[test]
    fn filter_sections_duplicate_headings_both_included() {
        let content = "### Rust\n\nFirst block.\n\n### Rust\n\nSecond block.\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("First block."));
        assert!(r.content.contains("Second block."));
    }

    #[test]
    fn filter_sections_nested_h4_included_in_body() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.content.contains("#### Sub-heading"), "h4 should be part of section body");
        assert!(r.content.contains("More Rust content."));
    }

    #[test]
    fn filter_sections_heading_whitespace_normalized() {
        // Double space after ### and trailing space
        let content = "###  Rust  \n\nContent.\n";
        let r = filter_sections(content, &["rust"]);
        assert!(r.matched, "should match despite whitespace");
        assert_eq!(r.available, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_no_headings_in_file_returns_not_matched() {
        let content = "Just a preamble\nno headings here\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(!r.matched);
        assert!(r.available.is_empty());
        assert_eq!(r.missing, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_indented_heading_not_a_boundary() {
        // Leading space — NOT a section boundary
        let content = "### Real\n\nBody.\n\n ### Fake\n\nNot a section.\n";
        let r = filter_sections(content, &["Real"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Real"]);
        // The indented line is part of the "Real" section body
        assert!(r.content.contains(" ### Fake"));
    }

    #[test]
    #[should_panic(expected = "precondition")]
    fn filter_sections_empty_sections_is_caller_error() {
        // debug_assert! fires in debug builds (including `cargo test`).
        // This test will NOT catch the precondition violation in `--release` builds.
        filter_sections("### Rust\nContent\n", &[]);
    }

    #[test]
    fn filter_sections_available_in_file_order() {
        let r = filter_sections(SAMPLE, &["Python"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib filter_sections 2>&1 | tail -20
```

Expected: compile succeeds; all tests in `filter_sections` panic with `not yet implemented`.

- [ ] **Step 3: Commit the failing tests**

```bash
git add src/memory/filter.rs
git commit -m "test(memory): failing unit tests for filter_sections"
```

---

### Task 3: Implement `filter_sections`

**Files:**
- Modify: `src/memory/filter.rs`

- [ ] **Step 1: Replace `todo!()` with the implementation**

```rust
pub fn filter_sections(content: &str, sections: &[&str]) -> FilterResult {
    debug_assert!(!sections.is_empty(), "precondition: sections must be non-empty");

    // --- Parse content into preamble + blocks ---
    // Each block: (normalized_heading, Vec of raw lines including the ### line)
    let mut preamble_lines: Vec<&str> = Vec::new();
    let mut blocks: Vec<(String, Vec<&str>)> = Vec::new();
    let mut in_preamble = true;

    for line in content.lines() {
        if line.starts_with("### ") {
            // Normalize: strip "### " prefix, trim leading+trailing whitespace.
            // The raw line is preserved in the block's line vec for output.
            let normalized = line["### ".len()..].trim().to_string();
            blocks.push((normalized, vec![line]));
            in_preamble = false;
        } else if in_preamble {
            preamble_lines.push(line);
        } else if let Some(block) = blocks.last_mut() {
            block.1.push(line);
        }
    }

    // available: normalized heading text of every block, in file order
    let available: Vec<String> = blocks.iter().map(|(h, _)| h.clone()).collect();

    // missing: requested sections with no match, in request order, caller casing
    let missing: Vec<String> = sections
        .iter()
        .filter(|&&s| !blocks.iter().any(|(h, _)| h.eq_ignore_ascii_case(s)))
        .map(|&s| s.to_string())
        .collect();

    // matched_lines: all lines from matching blocks, in file order
    let matched_lines: Vec<&str> = blocks
        .iter()
        .filter(|(h, _)| sections.iter().any(|s| s.eq_ignore_ascii_case(h)))
        .flat_map(|(_, lines)| lines.iter().copied())
        .collect();

    let matched = !matched_lines.is_empty();

    // Reconstruct output: preamble + matched section lines, joined by "\n".
    // Append "\n" if the original content ended with a newline (lines() strips it).
    let output: Vec<&str> = preamble_lines.iter().copied().chain(matched_lines).collect();
    let mut result_content = output.join("\n");
    if content.ends_with('\n') {
        result_content.push('\n');
    }

    FilterResult { content: result_content, matched, missing, available }
}
```

- [ ] **Step 2: Run the unit tests**

```bash
cargo test --lib filter_sections 2>&1 | tail -20
```

Expected: all tests in `src/memory/filter.rs` pass.

- [ ] **Step 3: Run full suite to confirm nothing else broke**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/memory/filter.rs
git commit -m "feat(memory): implement filter_sections"
```

---

### Task 4: Add `sections` param to `Memory` input schema

**Files:**
- Modify: `src/tools/memory.rs`

- [ ] **Step 1: Add `sections` to the schema**

In the `input_schema` method of `impl Tool for Memory`, add `sections` to `properties` after the `topic` entry:

Old text:
```rust
                "topic": {
                    "type": "string",
                    "description": "For read/write/delete/refresh_anchors. Path-like key, e.g. 'architecture'."
                },
```

New text:
```rust
                "topic": {
                    "type": "string",
                    "description": "For read/write/delete/refresh_anchors. Path-like key, e.g. 'architecture'."
                },
                "sections": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "For read. Return only the listed ### headings (case-insensitive). E.g. [\"Rust\", \"TypeScript\"]. Omit to return full content."
                },
```

- [ ] **Step 2: Verify compile + tests still pass**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tools/memory.rs
git commit -m "feat(memory): add sections param to Memory input schema"
```

---

### Task 5: Integrate filter into the read arm + integration tests

**Files:**
- Modify: `src/tools/memory.rs`

- [ ] **Step 1: Write failing integration tests**

At the bottom of the test module in `src/tools/memory.rs`, add:

```rust
    #[tokio::test]
    async fn memory_read_sections_filter_integration() {
        let (_dir, ctx) = test_ctx_with_project().await;

        // Write a multi-section memory
        let content = "# Lang Patterns\n\nIntro.\n\n### Rust\n\nRust stuff.\n\n### TypeScript\n\nTS stuff.\n";
        Memory.call(json!({ "action": "write", "topic": "language-patterns", "content": content }), &ctx).await.unwrap();

        // Filter to Rust only
        let result = Memory.call(json!({ "action": "read", "topic": "language-patterns", "sections": ["Rust"] }), &ctx).await.unwrap();
        let text = result["content"].as_str().unwrap();
        assert!(text.contains("### Rust"), "should contain Rust section");
        assert!(text.contains("Rust stuff."));
        assert!(!text.contains("### TypeScript"), "should not contain TypeScript");
        assert!(text.contains("# Lang Patterns"), "should contain preamble");

        // Empty sections array → full content (same as omitting the param)
        let result = Memory.call(json!({ "action": "read", "topic": "language-patterns", "sections": [] }), &ctx).await.unwrap();
        let text = result["content"].as_str().unwrap();
        assert!(text.contains("### Rust") && text.contains("### TypeScript"), "empty sections = full content");

        // Unknown section → RecoverableError; hint lists available sections.
        // Tool::call returns Err(RecoverableError) directly — route_tool_error is
        // only invoked by the MCP server, not in unit tests.
        let err = Memory.call(json!({ "action": "read", "topic": "language-patterns", "sections": ["Go"] }), &ctx).await.unwrap_err();
        let rec = err.downcast_ref::<RecoverableError>().expect("should be RecoverableError");
        let hint = rec.hint.as_deref().unwrap_or("");
        assert!(hint.contains("Rust") && hint.contains("TypeScript"), "hint should list available sections: {hint}");

        // Partial match → content + missing list
        let result = Memory.call(json!({ "action": "read", "topic": "language-patterns", "sections": ["Rust", "Go"] }), &ctx).await.unwrap();
        assert!(result["content"].as_str().is_some(), "matched sections should be in content");
        let missing = result["missing"].as_array().expect("missing field should be present");
        assert_eq!(missing, &[json!("Go")]);
    }

    #[tokio::test]
    async fn memory_read_sections_filter_private_integration() {
        let (_dir, ctx) = test_ctx_with_project().await;

        // Write a private multi-section memory
        let content = "### Rust\n\nRust stuff.\n\n### Python\n\nPython stuff.\n";
        Memory.call(json!({ "action": "write", "topic": "lang", "content": content, "private": true }), &ctx).await.unwrap();

        // Filtering applies in the private branch too
        let result = Memory.call(json!({ "action": "read", "topic": "lang", "sections": ["Rust"], "private": true }), &ctx).await.unwrap();
        let text = result["content"].as_str().unwrap();
        assert!(text.contains("### Rust"), "should contain Rust");
        assert!(!text.contains("### Python"), "should not contain Python");
    }
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test memory_read_sections_filter 2>&1 | tail -20
```

Expected: tests compile but fail (sections param parsed but filtering not wired yet).

- [ ] **Step 3: Add the `apply_sections_filter` helper**

Add this free function just **above** `impl Tool for Memory` in `src/tools/memory.rs`:

```rust
/// Apply `sections` filtering to memory content and produce the JSON response value.
///
/// - If `sections` is empty, returns `content` unchanged (no filtering).
/// - If filtering is active and nothing matched, returns a `RecoverableError`.
/// - Handles the inline-vs-buffer threshold; uses a `@`-prefixed synthetic path
///   when buffering filtered content so `store_file` does not stat a missing file
///   and evict the entry immediately.
fn apply_sections_filter(
    content: String,
    topic: &str,
    sections: &[String],
    output_buffer: &Arc<crate::tools::output_buffer::OutputBuffer>,
) -> anyhow::Result<serde_json::Value> {
    let (content, missing) = if sections.is_empty() {
        (content, vec![])
    } else {
        let section_refs: Vec<&str> = sections.iter().map(String::as_str).collect();
        let result = crate::memory::filter::filter_sections(&content, &section_refs);
        if !result.matched {
            let hint = if result.available.is_empty() {
                "this memory has no ### sections to filter".to_string()
            } else {
                format!("available sections: {}", result.available.join(", "))
            };
            return Err(RecoverableError::with_hint("no sections matched", hint).into());
        }
        (result.content, result.missing)
    };

    let value = if crate::tools::exceeds_inline_limit(&content) {
        let total_lines = content.lines().count();
        // Use a `@`-prefixed synthetic path: store_file sets source_path=None for
        // paths starting with '@', preventing get_with_refresh_flag from stat-ing
        // a non-existent file and immediately evicting the entry.
        let synthetic_path = format!("@memory:{topic}:filtered");
        let file_id = output_buffer.store_file(synthetic_path, content);
        if missing.is_empty() {
            json!({ "file_id": file_id, "total_lines": total_lines })
        } else {
            json!({ "file_id": file_id, "total_lines": total_lines, "missing": missing })
        }
    } else if missing.is_empty() {
        json!({ "content": content })
    } else {
        json!({ "content": content, "missing": missing })
    };

    Ok(value)
}
```

> **Note:** `Arc` is already imported at the top of `src/tools/memory.rs`. If it is not, add `use std::sync::Arc;`.

- [ ] **Step 4: Wire the filter into the read arm**

In `Memory::call`, at the top of the `"read" =>` arm, add sections parsing right after the `topic` and `private` lines:

```rust
            "read" => {
                let topic = super::require_str_param(&input, "topic")?;
                let private = parse_bool_param(&input["private"]);
                let sections: Vec<String> = input["sections"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
```

Then in the **private branch**, clone the `Arc` before the closure (to avoid capturing `ctx` through two paths) and replace the `Some(content) =>` block:

```rust
                if private {
                    let buf = Arc::clone(&ctx.output_buffer);
                    ctx.agent
                        .with_project(|p| {
                            match p.private_memory.read(topic)? {
                                Some(content) => apply_sections_filter(content, topic, &sections, &buf),
                                None => Err(RecoverableError::with_hint(
                                    format!("topic '{}' not found", topic),
                                    "Use memory(action='list') to see available topics",
                                )
                                .into()),
                            }
                        })
                        .await
```

In the **shared branch**, replace the `Some(content) =>` block similarly (no closure here, direct call):

```rust
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    let store = crate::memory::MemoryStore::from_dir(memories_dir)?;
                    match store.read(topic)? {
                        Some(content) => apply_sections_filter(content, topic, &sections, &ctx.output_buffer),
                        None => Err(RecoverableError::with_hint(
                            format!("topic '{}' not found", topic),
                            "Use memory(action='list') to see available topics",
                        )
                        .into()),
                    }
                }
```

The `None =>` error branches and everything outside `Some(content) =>` are unchanged.

- [ ] **Step 5: Run integration tests**

```bash
cargo test memory_read_sections_filter 2>&1 | tail -20
```

Expected: both integration tests pass.

- [ ] **Step 6: Run full suite**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -10
```

Expected: all tests pass, no warnings.

- [ ] **Step 7: Commit**

```bash
git add src/tools/memory.rs
git commit -m "feat(memory): wire sections filter into Memory read arm"
```

---

### Task 6: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Update the `action="read"` line**

Find:
```
  - `action="read"` — requires `topic`. Pass `private=true` for private store.
```

Replace with:
```
  - `action="read"` — requires `topic`. Pass `private=true` for private store. Pass `sections: ["Rust", "TypeScript"]` to return only the listed `### Heading` blocks (case-insensitive).
```

- [ ] **Step 2: Update the `language-patterns` rule**

Find:
```
6. **Read `language-patterns` memory before writing or editing code.** `memory(action="read", topic="language-patterns")` contains per-language anti-patterns and correct patterns. Consult it before code changes or code review.
```

Replace with:
```
6. **Read `language-patterns` memory before writing or editing code.** `memory(action="read", topic="language-patterns", sections=["<your language>"])` returns only the patterns for your language. Consult it before code changes or code review.
```

- [ ] **Step 3: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs(memory): document sections filter param"
```

---

### Task 7: Final verification

- [ ] **Step 1: Full suite**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -15
```

Expected: all tests pass, no fmt diffs, no clippy warnings.

- [ ] **Step 2: Smoke test via MCP (recommended)**

Restart the MCP server with `/mcp` after `cargo build --release`, then verify:
```
memory(action="read", topic="language-patterns", sections=["Rust"])
```
returns only the Rust section of your `language-patterns` memory.
