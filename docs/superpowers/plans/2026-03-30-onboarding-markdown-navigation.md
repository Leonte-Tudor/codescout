# Onboarding Markdown Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `@tool_*` buffer storage of the onboarding prompt with a `.codescout/tmp/onboarding-prompt.md` file, navigable via `read_markdown` heading-based sections.

**Architecture:** `call_content()` writes the prompt to a temp markdown file, builds a heading map from the content, and returns structured JSON with the file path, section list, and `read_markdown`-based instructions. Instruction builders change from `read_file("@tool_xxx")` to `read_markdown(path, heading="...")`.

**Tech Stack:** Rust, `std::fs` for file writing, string scanning for heading extraction

---

### Task 1: Add `build_heading_map` helper and update instruction builders

**Files:**
- Modify: `src/tools/workflow.rs` (add helper, update `build_buffered_onboarding_instructions` and `build_buffered_refresh_instructions`)

- [ ] **Step 1: Write the failing test for `build_heading_map`**

Add to `mod tests` in `src/tools/workflow.rs`:

```rust
#[test]
fn build_heading_map_extracts_level2_headings() {
    let prompt = "\
# Title

Intro text.

## Phase 1: Explore
Step 1 details.
Step 2 details.
More lines.

## Phase 2: Write Memories
Memory A.
Memory B.

## After Everything
Final notes.
";
    let sections = build_heading_map(prompt);
    assert_eq!(sections.len(), 3);
    assert!(sections[0].contains("Phase 1: Explore"));
    assert!(sections[0].contains("lines)"));
    assert!(sections[1].contains("Phase 2: Write Memories"));
    assert!(sections[2].contains("After Everything"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_heading_map_extracts_level2_headings`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement `build_heading_map`**

Add near the other instruction builder helpers (~line 935):

```rust
/// Extract level-2 headings from markdown content with line counts.
fn build_heading_map(prompt: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt.lines().collect();
    let mut headings: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            headings.push((line.to_string(), i));
        }
    }
    headings
        .iter()
        .enumerate()
        .map(|(idx, (heading, start))| {
            let end = headings
                .get(idx + 1)
                .map(|(_, s)| *s)
                .unwrap_or(lines.len());
            format!("{} ({} lines)", heading, end - start)
        })
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test build_heading_map_extracts_level2_headings`
Expected: PASS

- [ ] **Step 5: Update instruction builder signatures and content**

Change `build_buffered_onboarding_instructions` signature from `(ref_id: &str, ...)` to `(prompt_path: &str, ...)` and update the instruction text to use `read_markdown`:

```rust
/// Build dispatch instructions for full onboarding, adapted to client capability.
fn build_buffered_onboarding_instructions(prompt_path: &str, subagent_capable: bool) -> String {
    if subagent_capable {
        format!(
            "\
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and \
memory writing. The subagent must read the onboarding prompt section by section:
  read_markdown(\"{prompt_path}\")  — see all sections
  read_markdown(\"{prompt_path}\", heading=\"## Phase 1: Explore the Code\")  — read a section

Do NOT read the entire prompt at once — navigate by heading.

When the subagent completes, report its summary to the user. Then read whichever \
memories are relevant to the user's current task via memory(action=\"read\", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite \
for all subsequent work.

If you cannot spawn subagents, read the prompt yourself section by section."
        )
    } else {
        format!(
            "\
Onboarding required — this project has not been explored yet.

Read the onboarding prompt section by section:
  read_markdown(\"{prompt_path}\")  — see all sections
  read_markdown(\"{prompt_path}\", heading=\"## Phase 1: Explore the Code\")  — read a section

Follow the instructions to explore the codebase and write project memories. \
Navigate by heading — do NOT read the entire file at once."
        )
    }
}
```

Do the same for `build_buffered_refresh_instructions` — change `ref_id` to `prompt_path`, update text to use `read_markdown`:

```rust
/// Build dispatch instructions for version refresh, adapted to client capability.
fn build_buffered_refresh_instructions(
    prompt_path: &str,
    stored: Option<u32>,
    current: u32,
    subagent_capable: bool,
) -> String {
    let stored_str = stored
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "pre-versioning".to_string());

    if subagent_capable {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Spawn a general-purpose subagent with model=sonnet to regenerate the system prompt. \
The subagent must read the refresh prompt:
  read_markdown(\"{prompt_path}\")

The subagent will re-read memories and regenerate system-prompt.md without \
re-exploring the codebase.

When the subagent completes, continue with the user's original task."
        )
    } else {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Read the refresh prompt: read_markdown(\"{prompt_path}\")
Follow it to re-read memories and regenerate system-prompt.md."
        )
    }
}
```

- [ ] **Step 6: Update instruction builder tests**

Update the 4 existing tests to check for `read_markdown` instead of `read_file`, and pass a file path instead of a `@tool_` ref:

```rust
#[test]
fn build_buffered_onboarding_instructions_claude() {
    let instructions =
        build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", true);
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"), "must contain path");
    assert!(instructions.contains("subagent"), "Claude instructions must mention subagent");
    assert!(instructions.contains("read_markdown"), "must reference read_markdown");
    assert!(!instructions.contains("read_file"), "must NOT reference read_file");
}

#[test]
fn build_buffered_onboarding_instructions_generic() {
    let instructions =
        build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", false);
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"), "must contain path");
    assert!(!instructions.contains("subagent"), "generic must NOT mention subagent");
    assert!(instructions.contains("read_markdown"), "must reference read_markdown");
}

#[test]
fn build_buffered_refresh_instructions_claude() {
    let instructions = build_buffered_refresh_instructions(
        ".codescout/tmp/onboarding-prompt.md", Some(1), 2, true,
    );
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
    assert!(instructions.contains("v1"));
    assert!(instructions.contains("v2"));
    assert!(instructions.contains("subagent"));
    assert!(instructions.contains("read_markdown"));
}

#[test]
fn build_buffered_refresh_instructions_generic() {
    let instructions = build_buffered_refresh_instructions(
        ".codescout/tmp/onboarding-prompt.md", None, 2, false,
    );
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
    assert!(instructions.contains("pre-versioning"));
    assert!(!instructions.contains("subagent"));
    assert!(instructions.contains("read_markdown"));
}
```

- [ ] **Step 7: Run all tests, fmt, clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All pass. Some `call_content` tests may fail — that's expected, they'll be fixed in Task 2.

- [ ] **Step 8: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add heading map builder, update instructions to use read_markdown"
```

---

### Task 2: Rewrite `call_content` to write temp file instead of buffer

**Files:**
- Modify: `src/tools/workflow.rs` (`Onboarding::call_content`)

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
#[tokio::test]
async fn onboarding_call_content_writes_markdown_file() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text)
        .expect("response must be valid JSON");

    // Must have prompt_path (not output_id)
    let prompt_path = parsed["prompt_path"].as_str().expect("must have prompt_path");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "path must reference the markdown file, got: {prompt_path}"
    );
    assert!(
        parsed.get("output_id").is_none(),
        "must NOT have output_id (old buffer pattern)"
    );

    // File must exist on disk
    let root = ctx.agent.project_root().await.unwrap();
    let full_path = root.join(prompt_path);
    assert!(full_path.exists(), "prompt file must exist at {}", full_path.display());

    // File must contain the onboarding prompt content
    let file_content = std::fs::read_to_string(&full_path).unwrap();
    assert!(
        file_content.contains("## Return Contract") || file_content.contains("## Phase"),
        "file must contain onboarding prompt content"
    );

    // Must have sections array
    let sections = parsed["sections"].as_array().expect("must have sections array");
    assert!(!sections.is_empty(), "sections must not be empty");
    let first = sections[0].as_str().unwrap_or("");
    assert!(
        first.contains("lines)"),
        "section entries must include line counts, got: {first}"
    );

    // Must have read_markdown in instructions
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_call_content_writes_markdown_file`
Expected: FAIL — response still has `output_id`, no `prompt_path`.

- [ ] **Step 3: Rewrite `call_content`**

Replace the `call_content` method body:

```rust
    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        let val = self.call(input, ctx).await?;

        // If there's a subagent prompt, write it to a temp markdown file
        // and return compact instructions with heading-based navigation.
        if let Some(prompt) = val["subagent_prompt"].as_str() {
            let compact = format_onboarding(&val);
            let root = ctx.agent.require_project_root().await?;

            // Write prompt to .codescout/tmp/onboarding-prompt.md
            let tmp_dir = root.join(".codescout").join("tmp");
            std::fs::create_dir_all(&tmp_dir)?;
            let prompt_path = tmp_dir.join("onboarding-prompt.md");
            std::fs::write(&prompt_path, prompt)?;

            let rel_path = ".codescout/tmp/onboarding-prompt.md";
            let sections = build_heading_map(prompt);
            let name = client_name(ctx);
            let subagent = is_subagent_capable(name.as_deref());

            let instructions =
                if val.get("version_stale").is_some() || val.get("explicit_refresh").is_some() {
                    let stored = val["stored_version"].as_u64().map(|v| v as u32);
                    let current = val["current_version"].as_u64().unwrap_or(0) as u32;
                    build_buffered_refresh_instructions(rel_path, stored, current, subagent)
                } else {
                    build_buffered_onboarding_instructions(rel_path, subagent)
                };

            let response = serde_json::json!({
                "prompt_path": rel_path,
                "summary": compact,
                "sections": sections,
                "instructions": instructions,
            });

            return Ok(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|_| format!("{{\"prompt_path\":\"{rel_path}\"}}")),
            )]);
        }

        // Single-block fast path: already-onboarded status.
        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }

        // Fallback
        let compact = format_onboarding(&val);
        Ok(vec![rmcp::model::Content::text(compact)])
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test onboarding_call_content_writes_markdown_file`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): write prompt to temp .md file with heading map"
```

---

### Task 3: Update existing `call_content` tests

**Files:**
- Modify: `src/tools/workflow.rs` (test functions)

- [ ] **Step 1: Update `onboarding_call_content_buffers_prompt_with_ref`**

Rename and update to check for file-based response:

```rust
#[tokio::test]
async fn onboarding_call_content_writes_prompt_file() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    // Must contain prompt_path
    assert!(
        text.contains("prompt_path"),
        "response must contain prompt_path, got: {}",
        &text[..text.len().min(200)]
    );

    // Must contain read_markdown instructions
    assert!(
        text.contains("read_markdown"),
        "response must contain read_markdown instructions"
    );

    // Must NOT contain the raw subagent prompt content
    assert!(
        !text.contains("## Return Contract"),
        "response must NOT contain raw prompt content"
    );
}
```

- [ ] **Step 2: Update `onboarding_call_content_returns_two_blocks`**

Replace assertions to check for `prompt_path` and `sections` instead of `output_id`:

```rust
#[tokio::test]
async fn onboarding_call_content_returns_structured_response() {
    // Test name kept for history; validates the structured JSON response.
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("block must be valid JSON");

    // prompt_path must reference the markdown file
    let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "prompt_path must reference markdown file, got: {prompt_path:?}"
    );

    // sections must be present and non-empty
    let sections = parsed["sections"].as_array().expect("must have sections");
    assert!(!sections.is_empty(), "sections must not be empty");

    // instructions must reference read_markdown, not read_file
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown"
    );
    assert!(
        instructions.contains(prompt_path),
        "instructions must reference the prompt path"
    );
}
```

- [ ] **Step 3: Update `onboarding_call_content_force_delivers_instructions`**

Replace `@tool_` assertions with `prompt_path` and `read_markdown` assertions:

```rust
#[tokio::test]
async fn onboarding_call_content_force_delivers_instructions() {
    let (_dir, ctx) = project_ctx().await;

    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(content.len(), 1);

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        !text.contains("[?]"),
        "call_content must not emit [?] placeholder, got: {text:?}"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("call_content block must be valid JSON");
    assert!(
        parsed["prompt_path"].as_str().is_some(),
        "must have prompt_path"
    );
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown") || instructions.contains("subagent"),
        "instructions must guide the agent, got: {instructions:?}"
    );
}
```

- [ ] **Step 4: Update version refresh test**

Read the current `onboarding_call_content_returns_two_blocks_for_version_refresh` test and update it to check for `prompt_path` and `read_markdown` instead of `output_id` and `read_file`.

- [ ] **Step 5: Update workspace test**

Read the current `onboarding_call_content_includes_workspace_info` test and update it similarly.

- [ ] **Step 6: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All pass, 0 failures.

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "test(onboarding): update call_content tests for markdown file navigation"
```

---

### Task 4: Add markdown tool mentions to onboarding prompt

**Files:**
- Modify: `src/prompts/onboarding_prompt.md`

- [ ] **Step 1: Find the tool reference section**

Search for where tools are listed in the onboarding prompt (likely near the preamble or in the tool usage instructions).

- [ ] **Step 2: Add markdown tool mentions**

Add `read_markdown` and `edit_markdown` alongside existing tool references. The exact location depends on what's already there, but the content should include:

```markdown
- `read_markdown(path)` — heading map for .md files; navigate by section
- `read_markdown(path, heading="## Section")` — read a specific section
- `edit_markdown(path, heading="## Section", content="...")` — edit a markdown section
```

These should go wherever the existing `read_file`, `list_symbols`, `find_symbol` etc. are mentioned.

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All pass (prompt content change doesn't break logic tests).

- [ ] **Step 4: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs(onboarding): add read_markdown and edit_markdown to tool references"
```

---

### Task 5: Build release and verify via MCP

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`

- [ ] **Step 2: Restart MCP server**

Run `/mcp` in Claude Code.

- [ ] **Step 3: Verify onboarding response**

Call `onboarding(force: true)` and verify:
1. Response has `prompt_path` (not `output_id`)
2. Response has `sections` array with heading names and line counts
3. `.codescout/tmp/onboarding-prompt.md` exists on disk
4. `read_markdown(".codescout/tmp/onboarding-prompt.md")` returns the heading map
5. `read_markdown(".codescout/tmp/onboarding-prompt.md", heading="## Phase 1: Explore the Code")` returns that section
