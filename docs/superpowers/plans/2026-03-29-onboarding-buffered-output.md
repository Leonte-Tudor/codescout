# Onboarding Buffered Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `Onboarding::call_content()` buffer the subagent prompt via the `@tool_*` system and return client-aware dispatch instructions, keeping the main agent's context small (~1-2KB) regardless of prompt size.

**Architecture:** Replace the two raw `Content::text` blocks with a single structured JSON response containing a `@tool_*` ref handle. Detect the MCP client from `ToolContext::peer` to tailor instructions (subagent dispatch for Claude Code, self-read for other clients).

**Tech Stack:** Rust, rmcp 1.3.0 (`Peer::peer_info()` → `ClientInfo`), existing `OutputBuffer::store_tool()`

---

### Task 1: Add client detection helper

**Files:**
- Modify: `src/tools/workflow.rs` (add helper function near line ~920, before `build_main_agent_instructions`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/tools/workflow.rs`:

```rust
#[test]
fn is_subagent_capable_detects_claude() {
    assert!(is_subagent_capable(Some("claude-code")));
    assert!(is_subagent_capable(Some("Claude Code")));
    assert!(is_subagent_capable(Some("claude-code-ide")));
    assert!(!is_subagent_capable(Some("cursor")));
    assert!(!is_subagent_capable(Some("copilot")));
    assert!(!is_subagent_capable(Some("windsurf")));
    assert!(!is_subagent_capable(None));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test is_subagent_capable_detects_claude`
Expected: FAIL — `is_subagent_capable` not found.

- [ ] **Step 3: Implement the helpers**

Add before `build_main_agent_instructions` in `src/tools/workflow.rs`:

```rust
/// Extract the MCP client name from the peer info (set during initialize handshake).
fn client_name(ctx: &ToolContext) -> Option<String> {
    ctx.peer
        .as_ref()
        .and_then(|p| p.peer_info())
        .map(|info| info.client_info.name.clone())
}

/// Returns true if the client is known to support subagent spawning.
/// Conservative: only Claude Code for now. Add others as they gain support.
fn is_subagent_capable(name: Option<&str>) -> bool {
    name.map_or(false, |n| n.to_lowercase().contains("claude"))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test is_subagent_capable_detects_claude`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add client detection helpers for subagent capability"
```

---

### Task 2: Add buffered instruction builders

**Files:**
- Modify: `src/tools/workflow.rs` (replace `build_main_agent_instructions` and `build_prompt_refresh_main_instructions`)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/tools/workflow.rs`:

```rust
#[test]
fn build_buffered_onboarding_instructions_claude() {
    let instructions = build_buffered_onboarding_instructions("@tool_abc123", true);
    assert!(instructions.contains("@tool_abc123"), "must contain the ref handle");
    assert!(instructions.contains("subagent"), "Claude instructions must mention subagent");
    assert!(instructions.contains("read_file"), "must tell how to read the buffer");
    assert!(!instructions.contains("Pass the content of the `subagent_prompt`"),
        "must NOT reference the old pass-through pattern");
}

#[test]
fn build_buffered_onboarding_instructions_generic() {
    let instructions = build_buffered_onboarding_instructions("@tool_abc123", false);
    assert!(instructions.contains("@tool_abc123"), "must contain the ref handle");
    assert!(!instructions.contains("subagent"), "generic instructions must NOT mention subagent");
    assert!(instructions.contains("read_file"), "must tell how to read the buffer");
}

#[test]
fn build_buffered_refresh_instructions_claude() {
    let instructions = build_buffered_refresh_instructions("@tool_abc123", Some(1), 2, true);
    assert!(instructions.contains("@tool_abc123"));
    assert!(instructions.contains("v1"));
    assert!(instructions.contains("v2"));
    assert!(instructions.contains("subagent"));
}

#[test]
fn build_buffered_refresh_instructions_generic() {
    let instructions = build_buffered_refresh_instructions("@tool_abc123", None, 2, false);
    assert!(instructions.contains("@tool_abc123"));
    assert!(instructions.contains("pre-versioning"));
    assert!(!instructions.contains("subagent"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test build_buffered_onboarding_instructions && cargo test build_buffered_refresh_instructions`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement the instruction builders**

Replace `build_main_agent_instructions` and `build_prompt_refresh_main_instructions` with:

```rust
/// Build dispatch instructions for full onboarding, adapted to client capability.
fn build_buffered_onboarding_instructions(ref_id: &str, subagent_capable: bool) -> String {
    if subagent_capable {
        format!("\
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and \
memory writing. The subagent must:
1. Call read_file(\"{ref_id}\") to get the full onboarding prompt
2. Follow the prompt instructions to explore the codebase and write memories
3. Return an exploration summary and list of memories written

Do NOT read the onboarding prompt yourself — let the subagent handle it.

When the subagent completes, report its summary to the user. Then read whichever \
memories are relevant to the user's current task via memory(action=\"read\", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite \
for all subsequent work.

If you cannot spawn subagents, read the prompt yourself with \
read_file(\"{ref_id}\", start_line=1, end_line=100) and follow it directly.")
    } else {
        format!("\
Onboarding required — this project has not been explored yet.

Read the onboarding prompt: read_file(\"{ref_id}\")
If too large, paginate: read_file(\"{ref_id}\", start_line=1, end_line=100)

Follow the instructions to explore the codebase and write project memories. \
The prompt contains a step-by-step guide for codebase exploration, memory writing, \
and system prompt generation.")
    }
}

/// Build dispatch instructions for version refresh, adapted to client capability.
fn build_buffered_refresh_instructions(
    ref_id: &str,
    stored: Option<u32>,
    current: u32,
    subagent_capable: bool,
) -> String {
    let stored_str = stored
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "pre-versioning".to_string());

    if subagent_capable {
        format!("\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Spawn a general-purpose subagent with model=sonnet to regenerate the system prompt. \
The subagent must call read_file(\"{ref_id}\") to get the refresh prompt, then follow it.

The subagent will re-read memories and regenerate system-prompt.md without \
re-exploring the codebase.

When the subagent completes, continue with the user's original task.")
    } else {
        format!("\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Read the refresh prompt: read_file(\"{ref_id}\")
Follow it to re-read memories and regenerate system-prompt.md.")
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test build_buffered_onboarding_instructions && cargo test build_buffered_refresh_instructions`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add buffered instruction builders with client-aware dispatch"
```

---

### Task 3: Rewrite `call_content` to buffer the subagent prompt

**Files:**
- Modify: `src/tools/workflow.rs` (`Onboarding::call_content`, lines ~1677-1718)

- [ ] **Step 1: Write the failing test for buffered output**

Add to the `tests` module in `src/tools/workflow.rs`:

```rust
#[tokio::test]
async fn onboarding_call_content_buffers_prompt_with_ref() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Must return exactly 1 block (not 2 raw blocks)
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    // Must contain a @tool_ ref handle
    assert!(
        text.contains("@tool_"),
        "response must contain a @tool_ buffer ref, got: {}",
        &text[..text.len().min(200)]
    );

    // Must contain read_file instructions
    assert!(
        text.contains("read_file"),
        "response must contain read_file instructions"
    );

    // Must NOT contain the raw subagent prompt content
    assert!(
        !text.contains("## Return Contract"),
        "response must NOT contain raw prompt content (should be in buffer)"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_call_content_buffers_prompt_with_ref`
Expected: FAIL — still returns 2 blocks.

- [ ] **Step 3: Rewrite `call_content`**

Replace the entire `call_content` method body in `impl Tool for Onboarding`:

```rust
    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        let val = self.call(input, ctx).await?;

        // If there's a subagent prompt, buffer it and return compact instructions.
        if let Some(prompt) = val["subagent_prompt"].as_str() {
            let compact = format_onboarding(&val);
            let ref_id = ctx.output_buffer.store_tool("onboarding", prompt.to_string());

            let name = client_name(ctx);
            let subagent = is_subagent_capable(name.as_deref());

            // Determine which instruction builder based on whether this is a
            // version refresh (has stored_version) or full onboarding.
            let instructions = if val.get("version_stale").is_some()
                || val.get("explicit_refresh").is_some()
            {
                let stored = val["stored_version"].as_u64().map(|v| v as u32);
                let current = val["current_version"].as_u64().unwrap_or(0) as u32;
                build_buffered_refresh_instructions(&ref_id, stored, current, subagent)
            } else {
                build_buffered_onboarding_instructions(&ref_id, subagent)
            };

            let response = serde_json::json!({
                "output_id": ref_id,
                "summary": compact,
                "instructions": instructions,
                "hint": format!("read_file(\"{ref_id}\", start_line=1, end_line=50) to start reading"),
            });

            return Ok(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|_| format!("{{\"output_id\":\"{ref_id}\"}}")),
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

Run: `cargo test onboarding_call_content_buffers_prompt_with_ref`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): buffer subagent prompt via @tool_* ref system"
```

---

### Task 4: Update existing `call_content` tests

**Files:**
- Modify: `src/tools/workflow.rs` (tests: `onboarding_call_content_returns_two_blocks`, `onboarding_call_content_returns_two_blocks_for_version_refresh`, `onboarding_call_content_force_delivers_instructions`, `onboarding_call_content_includes_workspace_info`)

- [ ] **Step 1: Update `onboarding_call_content_returns_two_blocks`**

This test currently asserts 2 blocks. Replace it:

```rust
#[tokio::test]
async fn onboarding_call_content_returns_buffered_response() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Now returns 1 structured block (not 2 raw blocks)
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 block with buffered ref"
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text)
        .expect("response must be valid JSON");

    // Must have output_id with @tool_ prefix
    let output_id = parsed["output_id"].as_str().expect("must have output_id");
    assert!(output_id.starts_with("@tool_"), "ref must start with @tool_");

    // Must have instructions
    let instructions = parsed["instructions"].as_str().expect("must have instructions");
    assert!(instructions.contains("read_file"), "instructions must mention read_file");

    // Buffer must contain the actual prompt content
    let entry = ctx.output_buffer.get(output_id).expect("buffer entry must exist");
    assert!(
        entry.stdout.contains("## Return Contract"),
        "buffered content must contain the onboarding prompt"
    );
}
```

- [ ] **Step 2: Update `onboarding_call_content_returns_two_blocks_for_version_refresh`**

Replace with:

```rust
#[tokio::test]
async fn onboarding_call_content_buffers_version_refresh() {
    let (_dir, ctx) = onboarded_project_ctx().await;

    // Write stale config
    let config_path = ctx
        .agent
        .with_project(|p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
            config.project.onboarding_version = None;
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, &toml_str)?;
            Ok(config_path)
        })
        .await
        .unwrap();
    ctx.agent.reload_config_if_project_toml(&config_path).await;

    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();

    assert_eq!(content.len(), 1, "version refresh must return 1 buffered block");

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text)
        .expect("response must be valid JSON");

    let output_id = parsed["output_id"].as_str().expect("must have output_id");
    assert!(output_id.starts_with("@tool_"));

    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("outdated") || instructions.contains("refresh"),
        "refresh instructions must mention version state, got: {instructions}"
    );
}
```

- [ ] **Step 3: Update `onboarding_call_content_force_delivers_instructions`**

Read the current test body first and update assertions from 2-block format to 1-block buffered JSON. The test should verify the `instructions` field in the parsed JSON contains dispatch-relevant content (not the raw subagent prompt).

- [ ] **Step 4: Update `onboarding_call_content_includes_workspace_info`**

Read the current test body first and update assertions. The `summary` field in the JSON response should contain workspace info (e.g., "workspace (N projects)").

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass, 0 failures.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "test(onboarding): update call_content tests for buffered output format"
```

---

### Task 5: Remove dead code and run full checks

**Files:**
- Modify: `src/tools/workflow.rs` (remove `build_main_agent_instructions`, `build_prompt_refresh_main_instructions`, and their tests)

- [ ] **Step 1: Remove `build_main_agent_instructions`**

Delete the function (lines ~922-953) and its test `main_agent_instructions_are_concise` (find with grep).

- [ ] **Step 2: Remove `build_prompt_refresh_main_instructions`**

Delete the function (lines ~990-1013) and its test `prompt_refresh_main_instructions_mention_version`.

- [ ] **Step 3: Remove `main_agent_instructions` from `call()` return values**

In `call()`, there are three places that build and return `"main_agent_instructions"`:
1. Full onboarding path (~line 1643)
2. Version refresh path (~line 1249)
3. Explicit refresh path (~line 1165)

Remove the `"main_agent_instructions"` key from all three `json!({...})` return values. `call()` still returns `"subagent_prompt"` — that's unchanged and used by `call_content()`.

- [ ] **Step 4: Run cargo fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: Clean — no warnings, no errors.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "refactor(onboarding): remove dead main_agent_instructions code"
```

---

### Task 6: Build release and verify via MCP

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`

- [ ] **Step 2: Restart MCP server**

Run: `/mcp` (in Claude Code) to restart the server with the new binary.

- [ ] **Step 3: Manual test — trigger onboarding**

Call `onboarding(force: true)` and verify:
1. Response is a single compact JSON block (~1-2KB)
2. Contains `output_id` with `@tool_*` ref
3. Contains `instructions` with `read_file` guidance
4. `read_file("@tool_xxx")` returns the full onboarding prompt

- [ ] **Step 4: Commit (if any fixups needed)**

```bash
git add -A && git commit -m "fix(onboarding): post-verification fixups"
```
