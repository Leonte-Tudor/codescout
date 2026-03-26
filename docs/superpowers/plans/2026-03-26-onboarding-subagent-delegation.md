# Onboarding Subagent Delegation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the `onboarding()` tool response so exploration + memory writing runs in a subagent instead of exhausting the main agent's context window.

**Architecture:** The `Onboarding::call` method's response JSON is restructured: `instructions` and `system_prompt_draft` move into a new `subagent_prompt` field (concatenation of preamble + existing prompt + draft + gathered data + epilogue). A new `main_agent_instructions` field contains ~200 tokens telling the main agent to dispatch a Sonnet subagent. `call_content` emits two Content blocks instead of one.

**Tech Stack:** Rust, serde_json, rmcp (MCP framework)

**Spec:** `docs/superpowers/specs/2026-03-26-onboarding-subagent-delegation-design.md`

---

### Task 1: Add `build_subagent_preamble()` and `build_subagent_epilogue()` functions

**Files:**
- Modify: `src/tools/workflow.rs` (add two new functions near `build_system_prompt_draft`)

- [ ] **Step 1: Write the failing test for preamble**

Add this test in the `#[cfg(test)] mod tests` block in `src/tools/workflow.rs`:

```rust
#[test]
fn subagent_preamble_contains_activate_project() {
    let preamble = build_subagent_preamble();
    assert!(
        preamble.contains("onboarding subagent"),
        "preamble must identify the subagent role"
    );
    assert!(
        preamble.contains("activate_project"),
        "preamble must instruct subagent to activate project"
    );
    assert!(
        preamble.contains("read_only: false"),
        "preamble must request write access"
    );
}
```

- [ ] **Step 2: Write the failing test for epilogue**

```rust
#[test]
fn subagent_epilogue_contains_return_contract() {
    let epilogue = build_subagent_epilogue();
    assert!(
        epilogue.contains("Exploration Summary"),
        "epilogue must define exploration summary format"
    );
    assert!(
        epilogue.contains("Memories Written"),
        "epilogue must request memory list"
    );
    assert!(
        epilogue.contains("activate_project"),
        "epilogue must instruct subagent to restore project state"
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test subagent_preamble_contains -- --nocapture && cargo test subagent_epilogue_contains -- --nocapture`
Expected: FAIL — `build_subagent_preamble` and `build_subagent_epilogue` not found

- [ ] **Step 4: Implement `build_subagent_preamble()`**

Add near `build_system_prompt_draft` (around line 1400 area) in `src/tools/workflow.rs`:

```rust
/// Build the preamble prepended to the onboarding prompt for the subagent.
/// Instructs the subagent to activate the project before following the exploration steps.
fn build_subagent_preamble() -> String {
    "\
You are an onboarding subagent for codescout. Your job is to thoroughly explore \
this codebase and write project memories that will be used by every future session.

FIRST ACTION: Call activate_project(\".\", read_only: false) to initialize the \
project context. All subsequent tool calls depend on this.

Then follow the exploration and memory-writing instructions below exactly.

---

"
    .to_string()
}
```

- [ ] **Step 5: Implement `build_subagent_epilogue()`**

Add immediately after `build_subagent_preamble`:

```rust
/// Build the epilogue appended to the onboarding prompt for the subagent.
/// Defines the return contract: what the subagent must include in its final response.
fn build_subagent_epilogue() -> String {
    "\

---

## Return Contract

When you have completed ALL exploration steps and written ALL memories, end your \
response with this structured summary:

**Exploration Summary:**
- What this system does (your own words, not the README's)
- The 5 most important types/modules (name, file, role)
- How a typical operation flows (concrete function names)
- What surprised you (things docs didn't mention)

**Memories Written:**
- List each memory topic you wrote (e.g., \"architecture\", \"conventions\", etc.)

**Warnings:**
- Any issues encountered (index not built, LSP failures, files that couldn't be read)
- Steps you couldn't fully complete and why

This summary is returned to the main agent and shown to the user. Make it \
informative but concise — aim for 300-500 tokens total.

LAST ACTION: Call activate_project(\".\") before returning to ensure the parent's \
project state is unchanged."
        .to_string()
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test subagent_preamble_contains -- --nocapture && cargo test subagent_epilogue_contains -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test`
Expected: All existing tests still pass

- [ ] **Step 8: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add subagent preamble and epilogue builders"
```

---

### Task 2: Add `build_main_agent_instructions()` function

**Files:**
- Modify: `src/tools/workflow.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn main_agent_instructions_contains_dispatch_command() {
    let instructions = build_main_agent_instructions();
    assert!(
        instructions.contains("subagent"),
        "must mention subagent dispatch"
    );
    assert!(
        instructions.contains("model=sonnet"),
        "must specify sonnet model"
    );
    assert!(
        instructions.contains("subagent_prompt"),
        "must reference subagent_prompt field"
    );
    assert!(
        instructions.contains("Do NOT"),
        "must include anti-rationalization guard"
    );
    assert!(
        instructions.contains("cannot spawn subagents"),
        "must include fallback for non-subagent clients"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test main_agent_instructions_contains -- --nocapture`
Expected: FAIL — `build_main_agent_instructions` not found

- [ ] **Step 3: Implement `build_main_agent_instructions()`**

Add after `build_subagent_epilogue`:

```rust
/// Build the short dispatch instructions for the main agent.
/// This is the only part the main agent needs to read — ~200 tokens.
fn build_main_agent_instructions() -> String {
    "\
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and \
memory writing. Pass the content of the `subagent_prompt` field as the subagent's \
task prompt. Do NOT read or summarize `subagent_prompt` yourself — pass it through \
as-is.

The subagent will:
1. Explore the codebase thoroughly (symbol surveys, code reading, data flow tracing)
2. Write project memories (architecture, conventions, gotchas, etc.)
3. Return an exploration summary and list of memories written

When the subagent completes, report its summary to the user. Then read whichever \
memories are relevant to the user's current task via memory(action=\"read\", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite \
for all subsequent work.

Do NOT attempt to perform the exploration yourself — it will exhaust your context \
window. The subagent handles it in isolation.

If the subagent fails, report the error to the user. Do NOT fall back to exploring \
inline — suggest the user re-run onboarding or check the MCP server status.

If you cannot spawn subagents, execute the subagent prompt directly — but be aware \
this will consume significant context."
        .to_string()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test main_agent_instructions_contains -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add main agent dispatch instructions builder"
```

---

### Task 3: Restructure `Onboarding::call` response JSON

**Files:**
- Modify: `src/tools/workflow.rs` — the `Ok(json!({...}))` block at the end of `Onboarding::call` (around line 1300-1324)

- [ ] **Step 1: Write the failing test for new response shape**

```rust
#[tokio::test]
async fn onboarding_returns_subagent_prompt_and_instructions() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // New fields must exist
    assert!(
        result.get("subagent_prompt").is_some(),
        "response must include subagent_prompt"
    );
    assert!(
        result["subagent_prompt"].is_string(),
        "subagent_prompt must be a string"
    );
    assert!(
        result.get("main_agent_instructions").is_some(),
        "response must include main_agent_instructions"
    );
    assert!(
        result["main_agent_instructions"].is_string(),
        "main_agent_instructions must be a string"
    );

    // Old fields must be gone
    assert!(
        result.get("instructions").is_none(),
        "instructions field must be removed (moved into subagent_prompt)"
    );
    assert!(
        result.get("system_prompt_draft").is_none(),
        "system_prompt_draft must be removed (moved into subagent_prompt)"
    );

    // subagent_prompt must contain the preamble, onboarding prompt content, and epilogue
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("activate_project"),
        "subagent_prompt must contain preamble"
    );
    assert!(
        prompt.contains("## Return Contract"),
        "subagent_prompt must contain epilogue"
    );
    // The onboarding prompt contains Phase 1 exploration steps
    assert!(
        prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
        "subagent_prompt must contain onboarding prompt body"
    );
    // System prompt draft must be embedded
    assert!(
        prompt.contains("## System Prompt Draft"),
        "subagent_prompt must contain system prompt draft section"
    );

    // Lightweight metadata must still be present
    assert!(result.get("languages").is_some());
    assert!(result.get("config_created").is_some());
    assert!(result.get("onboarded").is_none() || !result["onboarded"].as_bool().unwrap_or(true));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_returns_subagent_prompt_and_instructions -- --nocapture`
Expected: FAIL — `subagent_prompt` field not found, `instructions` still present

- [ ] **Step 3: Modify `Onboarding::call` response assembly**

In the `Ok(json!({...}))` block at the end of `Onboarding::call`, replace the current response with:

```rust
        // Build the subagent prompt by concatenating preamble + onboarding prompt +
        // system prompt draft + gathered data + epilogue
        let subagent_prompt = {
            let mut sp = build_subagent_preamble();
            sp.push_str(&prompt);
            if !system_prompt_draft.is_empty() {
                sp.push_str("\n\n## System Prompt Draft\n\n");
                sp.push_str(&system_prompt_draft);
            }
            if let Some(suggestion) = features_suggestion {
                sp.push_str(&format!("\n\n> {suggestion}"));
            }
            // Append gathered data that the subagent needs
            sp.push_str("\n\n## Gathered Data\n\n");
            sp.push_str(&format!("**Hardware:** {}\n\n", serde_json::to_string_pretty(&hw).unwrap_or_default()));
            sp.push_str(&format!("**Model options:** {}\n\n", serde_json::to_string_pretty(&model_options).unwrap_or_default()));
            if !protected_memories.is_null() {
                sp.push_str(&format!("**Protected memories:** {}\n\n", serde_json::to_string_pretty(&protected_memories).unwrap_or_default()));
            }
            if workspace_mode {
                if let Some(ref ppm) = per_project_protected {
                    if !ppm.is_null() {
                        sp.push_str(&format!("**Per-project protected memories:** {}\n\n", serde_json::to_string_pretty(ppm).unwrap_or_default()));
                    }
                }
            }
            sp.push_str(&build_subagent_epilogue());
            sp
        };

        let main_agent_instructions = build_main_agent_instructions();

        Ok(json!({
            "languages": lang_list,
            "top_level": top_level,
            "config_created": created_config,
            "has_readme": gathered.readme_path.is_some(),
            "has_claude_md": gathered.claude_md_exists,
            "build_file": gathered.build_file_name,
            "entry_points": gathered.entry_points,
            "test_dirs": gathered.test_dirs,
            "ci_files": gathered.ci_files,
            "features_md": gathered.features_md,
            "index_status": index_status,
            "workspace_mode": workspace_mode,
            "projects": discovered_projects,
            "subagent_prompt": subagent_prompt,
            "main_agent_instructions": main_agent_instructions,
        }))
```

Note the removed fields: `instructions`, `system_prompt_draft`, `features_suggestion`, `hardware`, `model_options`, `protected_memories`, `per_project_protected_memories`. All moved into `subagent_prompt`.

- [ ] **Step 4: Run new test to verify it passes**

Run: `cargo test onboarding_returns_subagent_prompt_and_instructions -- --nocapture`
Expected: PASS

- [ ] **Step 5: Update existing tests that check removed fields**

The following tests reference `instructions` or `system_prompt_draft` and need updating:

**`onboarding_returns_instruction_prompt` (line ~2605):** Change to check `subagent_prompt` instead of `instructions`:

```rust
#[tokio::test]
async fn onboarding_returns_instruction_prompt() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("## Rules"));
    assert!(prompt.contains("## Memories to Create"));
    assert!(prompt.contains("rust")); // detected language
}
```

**`onboarding_includes_system_prompt_draft_field` (line ~2952):** Change to verify `system_prompt_draft` is inside `subagent_prompt`, not a top-level field:

```rust
#[tokio::test]
async fn onboarding_includes_system_prompt_draft_in_subagent_prompt() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), "# Test Project\nA test.").unwrap();
    std::fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // system_prompt_draft is no longer a top-level field
    assert!(
        result.get("system_prompt_draft").is_none(),
        "system_prompt_draft should not be a top-level field"
    );
    // But it should be embedded in subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("## System Prompt Draft"),
        "subagent_prompt should contain system prompt draft"
    );
}
```

**`onboarding_returns_gathered_context_fields` (line ~2919):** Update to not check for `instructions` as a top-level field. Check that `subagent_prompt` and `main_agent_instructions` exist instead.

**`onboarding_includes_hardware_and_model_options` (line ~4576):** Hardware and model_options are now inside `subagent_prompt`. Update to check `subagent_prompt` contains hardware info rather than checking top-level fields.

**`onboarding_includes_protected_memories_for_existing_topic` (line ~4617) and related protected memory tests:** Protected memories are now inside `subagent_prompt`. Update assertions to check the prompt string contains the memory data.

**`onboarding_includes_workspace_mode_and_per_project_protected` (line ~4991):** `per_project_protected_memories` moved into `subagent_prompt`. Update accordingly.

**`single_project_onboarding_unchanged` (line ~4950):** References `result["instructions"]`. Change to `result["subagent_prompt"]`.

**`workspace_onboarding_full_flow` (line ~5038):** References both `result["instructions"]` (line ~5064) and `result["system_prompt_draft"]` (line ~5075). Change both to assertions against `result["subagent_prompt"]`. Also update the `call_content` section of this test: change from expecting 1 content block to expecting 2, and verify workspace content is in block 2 (the subagent prompt), not block 1.

**`onboarding_discovers_sub_projects` (line ~4087):** References `result["system_prompt_draft"]` (line ~4136). Change to check `result["subagent_prompt"]` contains the system prompt draft content.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests pass (some tests updated, no regressions)

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): restructure response with subagent_prompt and main_agent_instructions"
```

---

### Task 4: Rewrite `call_content` for two-block delivery

**Files:**
- Modify: `src/tools/workflow.rs` — `Onboarding::call_content` method (line ~1326-1370)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_call_content_returns_two_blocks() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Must return exactly 2 content blocks
    assert_eq!(
        content.len(),
        2,
        "call_content must return 2 blocks (instructions + subagent prompt)"
    );

    // Block 1: main agent instructions
    let block1 = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        block1.contains("subagent"),
        "block 1 must contain dispatch instructions"
    );
    assert!(
        !block1.contains("## Return Contract"),
        "block 1 must NOT contain the subagent prompt content"
    );

    // Block 2: subagent prompt with delimiter
    let block2 = content[1].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        block2.contains("--- ONBOARDING SUBAGENT PROMPT"),
        "block 2 must start with delimiter"
    );
    assert!(
        block2.contains("## Return Contract"),
        "block 2 must contain the epilogue"
    );
    assert!(
        block2.contains("Explore the Code") || block2.contains("Memories to Create"),
        "block 2 must contain the onboarding exploration instructions"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_call_content_returns_two_blocks -- --nocapture`
Expected: FAIL — still returns 1 block

- [ ] **Step 3: Rewrite `call_content`**

Replace the entire `call_content` method body with:

```rust
    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        let val = self.call(input, ctx).await?;

        // Fast path: already onboarded — return status message as single block
        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }

        // Full onboarding: two content blocks
        // Block 1: compact metadata + main agent dispatch instructions
        let compact = format_onboarding(&val);
        let instructions = val["main_agent_instructions"].as_str().unwrap_or("");
        let block1 = format!("{}\n\n{}", compact, instructions);

        // Block 2: subagent prompt with delimiter (pass-through blob)
        let subagent_prompt = val["subagent_prompt"].as_str().unwrap_or("");
        let block2 = format!(
            "--- ONBOARDING SUBAGENT PROMPT (pass as-is to subagent) ---\n\n{}",
            subagent_prompt
        );

        Ok(vec![
            rmcp::model::Content::text(block1),
            rmcp::model::Content::text(block2),
        ])
    }
```

- [ ] **Step 4: Run new test to verify it passes**

Run: `cargo test onboarding_call_content_returns_two_blocks -- --nocapture`
Expected: PASS

- [ ] **Step 5: Update existing `call_content` test**

Update `onboarding_call_content_force_delivers_instructions` (line ~2742) to match the new two-block shape:

```rust
#[tokio::test]
async fn onboarding_call_content_force_delivers_instructions() {
    let (_dir, ctx) = project_ctx().await;

    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(content.len(), 2, "must return 2 content blocks");

    // Block 1 has dispatch instructions
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("subagent"),
        "block 1 must contain dispatch instructions, got: {text:?}"
    );

    // Block 2 has the full onboarding prompt
    let prompt = content[1].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        prompt.contains("## Rules") || prompt.contains("## Memories to Create"),
        "block 2 must contain onboarding exploration instructions"
    );
}
```

Update `onboarding_call_content_includes_workspace_info` (line ~4965) similarly — check block 2 for workspace content instead of the single block.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): rewrite call_content for two-block subagent delivery"
```

---

### Task 5: Update `format_onboarding` and verify prompt surfaces

**Files:**
- Modify: `src/tools/workflow.rs` — `format_onboarding` function (line ~1578)
- Review: `src/prompts/server_instructions.md`

- [ ] **Step 1: Verify `format_onboarding` still works**

`format_onboarding` reads `languages`, `config_created`, `workspace_mode`, `projects` — all still present in the new response. No changes needed to the function itself. Verify by running:

Run: `cargo test format_onboarding -- --nocapture` (if such a test exists) or `cargo test onboarding -- --nocapture`
Expected: PASS — the function works unchanged

- [ ] **Step 2: Review `server_instructions.md`**

Read `src/prompts/server_instructions.md` line 145. The current description is:
```
- `onboarding` — project discovery: detect languages, read key files, generate system
  prompt draft. Use `force=true` to re-scan.
```

This mentions "generate system prompt draft" which is still true (it's generated, just inside `subagent_prompt` now). The description is generic enough — **no change needed**.

- [ ] **Step 3: Run `cargo fmt` and `cargo clippy`**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: Clean — no warnings

- [ ] **Step 4: Run full test suite one final time**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit any formatting fixes**

```bash
git add -A
git commit -m "chore(onboarding): fmt + clippy cleanup"
```

(Skip this commit if no changes were made.)

---

### Task 6: Manual E2E verification

**Files:**
- None (testing only)

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: Build succeeds

- [ ] **Step 2: Restart MCP server**

Run: `/mcp` (in Claude Code) to restart the MCP server with the new binary.

- [ ] **Step 3: Test on a fixture project**

Run `onboarding(force=true)` against one of the test fixture projects (e.g., `tests/fixtures/rust-library`). Verify:
- Response contains `subagent_prompt` and `main_agent_instructions`
- Response does NOT contain `instructions` or `system_prompt_draft` at top level
- `main_agent_instructions` mentions "subagent" and "model=sonnet"
- `subagent_prompt` contains the preamble ("activate_project"), exploration steps, and epilogue ("Return Contract")

- [ ] **Step 4: Test fast path**

Run `onboarding()` (without force) on a project that's already onboarded. Verify:
- Response has `onboarded: true` with the familiar memory listing
- No `subagent_prompt` field — unchanged behavior

- [ ] **Step 5: Test actual subagent dispatch**

In a fresh session on a test project, let the main agent receive the onboarding response and verify it dispatches a subagent rather than exploring inline.

- [ ] **Step 6: Final commit with all changes**

If any fixes were needed during E2E testing:
```bash
git add -A
git commit -m "fix(onboarding): E2E verification fixes"
```
