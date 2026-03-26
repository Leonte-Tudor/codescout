# Onboarding Versioning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect stale system prompts after tool API changes and auto-refresh them via a lightweight subagent — no full re-exploration needed.

**Architecture:** A compiled `ONBOARDING_VERSION` constant compared against a stored version in `project.toml`. When stale, the `onboarding()` fast path dispatches a lightweight subagent to regenerate the system prompt from existing memories. An optimistic server-side version write prevents infinite re-trigger loops.

**Tech Stack:** Rust, serde/toml, rmcp (MCP framework)

**Spec:** `docs/superpowers/specs/2026-03-26-onboarding-versioning-design.md`

---

### Task 1: Add `onboarding_version` field to `ProjectSection`

**Files:**
- Modify: `src/config/project.rs` — `ProjectSection` struct (line ~23-35)

- [ ] **Step 1: Write the failing test for deserialization**

Add in `src/config/project.rs` test module (or create one if absent). If there's no test module, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_section_deserializes_onboarding_version() {
        let toml_with_version = r#"
            name = "test"
            languages = ["rust"]
            onboarding_version = 2
        "#;
        let section: ProjectSection = toml::from_str(toml_with_version).unwrap();
        assert_eq!(section.onboarding_version, Some(2));
    }

    #[test]
    fn project_section_deserializes_without_onboarding_version() {
        let toml_without = r#"
            name = "test"
            languages = ["rust"]
        "#;
        let section: ProjectSection = toml::from_str(toml_without).unwrap();
        assert_eq!(section.onboarding_version, None);
    }

    #[test]
    fn project_section_serializes_onboarding_version() {
        let section = ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: Some(1),
        };
        let toml_str = toml::to_string_pretty(&section).unwrap();
        assert!(toml_str.contains("onboarding_version = 1"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib project_section_deserializes -- --nocapture`
Expected: FAIL — `onboarding_version` field not found on `ProjectSection`

- [ ] **Step 3: Add the field to `ProjectSection`**

In `src/config/project.rs`, add to the `ProjectSection` struct after the `system_prompt` field:

```rust
    /// Tracks which ONBOARDING_VERSION was used to generate the system prompt.
    /// `None` means pre-versioning — treated as stale.
    #[serde(default)]
    pub onboarding_version: Option<u32>,
```

- [ ] **Step 4: Update `ProjectConfig::default_for` to include the new field**

Find `default_for` in `src/config/project.rs` — it constructs a `ProjectSection`. Add `onboarding_version: None` to the struct literal.

- [ ] **Step 5: Update ALL `ProjectSection` construction sites**

Search for all `ProjectSection {` in the codebase — there are multiple construction sites (onboarding config creation, `default_for`, tests). Add `onboarding_version: None` to each one.

In `src/tools/workflow.rs` around line 1143, where `ProjectSection` is constructed during config creation, add `onboarding_version: None`:

```rust
                project: crate::config::project::ProjectSection {
                    name,
                    languages: langs,
                    encoding: "utf-8".into(),
                    system_prompt: None,
                    tool_timeout_secs: 60,
                    onboarding_version: None,
                },
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib project_section -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test --lib`
Expected: All pass (no regressions)

- [ ] **Step 8: Commit**

```bash
git add src/config/project.rs src/tools/workflow.rs
git commit -m "feat(onboarding): add onboarding_version field to ProjectSection"
```

---

### Task 2: Add `ONBOARDING_VERSION` constant and version check helper

**Files:**
- Modify: `src/tools/workflow.rs`

- [ ] **Step 1: Write the failing test for version staleness check**

Add in the tests module in `src/tools/workflow.rs`:

```rust
#[test]
fn version_needs_refresh_when_none() {
    assert!(onboarding_version_stale(None));
}

#[test]
fn version_needs_refresh_when_old() {
    assert!(onboarding_version_stale(Some(0)));
}

#[test]
fn version_current_when_equal() {
    assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION)));
}

#[test]
fn version_current_when_newer_than_compiled() {
    assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION + 1)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib version_needs_refresh -- --nocapture`
Expected: FAIL — `ONBOARDING_VERSION` and `onboarding_version_stale` not found

- [ ] **Step 3: Implement the constant and helper**

Add near the top of `src/tools/workflow.rs` (near other constants, before the tool structs):

```rust
/// Bump this when system prompt surfaces change significantly.
/// Missing or lower stored version triggers auto-refresh of the system prompt.
/// See CLAUDE.md § "Onboarding Version" for when to bump.
const ONBOARDING_VERSION: u32 = 1;

/// Returns true if the stored onboarding version is stale (needs refresh).
/// `None` means pre-versioning project — always stale.
/// Stored > compiled (downgrade) is treated as current to avoid churn.
fn onboarding_version_stale(stored: Option<u32>) -> bool {
    match stored {
        None => true,
        Some(v) => v < ONBOARDING_VERSION,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib version_needs_refresh -- --nocapture && cargo test --lib version_current -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add ONBOARDING_VERSION constant and staleness check"
```

---

### Task 3: Add `build_prompt_refresh_subagent_prompt()` and `build_prompt_refresh_main_instructions()`

**Files:**
- Modify: `src/tools/workflow.rs`

- [ ] **Step 1: Write the failing test for refresh subagent prompt**

```rust
#[test]
fn prompt_refresh_subagent_prompt_contains_memory_reads() {
    let topics = vec!["architecture".to_string(), "conventions".to_string()];
    let prompt = build_prompt_refresh_subagent_prompt(&topics);
    assert!(prompt.contains("activate_project"), "must activate project");
    assert!(prompt.contains("architecture"), "must include memory topics");
    assert!(prompt.contains("conventions"), "must include memory topics");
    assert!(prompt.contains("system-prompt.md"), "must read current system prompt");
    assert!(prompt.contains("Do NOT re-explore"), "must forbid exploration");
    assert!(prompt.contains("activate_project"), "epilogue must restore state");
}

#[test]
fn prompt_refresh_main_instructions_contains_version_info() {
    let instructions = build_prompt_refresh_main_instructions(Some(1), ONBOARDING_VERSION);
    assert!(instructions.contains("subagent"), "must mention subagent");
    assert!(instructions.contains("model=sonnet"), "must specify model");
    assert!(instructions.contains("subagent_prompt"), "must reference prompt field");
    assert!(instructions.contains("1"), "must mention old version");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompt_refresh_subagent -- --nocapture && cargo test --lib prompt_refresh_main -- --nocapture`
Expected: FAIL — functions not found

- [ ] **Step 3: Implement `build_prompt_refresh_subagent_prompt()`**

Add after the existing `build_main_agent_instructions()` function:

```rust
/// Build the lightweight subagent prompt for system prompt refresh.
/// Instructs the subagent to read existing memories and regenerate the system prompt
/// without re-exploring the codebase.
fn build_prompt_refresh_subagent_prompt(memory_topics: &[String]) -> String {
    let topic_list = memory_topics
        .iter()
        .map(|t| format!("   - memory(action=\"read\", topic=\"{t}\")"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\
You are a system prompt refresh subagent for codescout. The project's tool API has \
been updated and the system prompt needs regenerating with current tool guidance.

FIRST ACTION: Call activate_project(\".\", read_only: false)

Then:
1. Read these existing memories to understand the project:
{topic_list}
2. Read the current system prompt: read_markdown(\".codescout/system-prompt.md\")
   If it does not exist, you will create it from scratch.
3. Regenerate the system prompt following this template:

# [Project Name] — Code Explorer Guidance

## Entry Points
[Where to start. Specific files + symbols, not module descriptions.]

## Key Abstractions
[3-5 core types with file paths. What to understand first.]

## Search Tips
[Concrete query examples that work well. Terms to avoid.]

## Navigation Strategy
[Recommended exploration order for new tasks.]

## Project Rules
[Conventions the AI should always follow.]

4. Write the updated system prompt: create_file(\".codescout/system-prompt.md\", ...)

Key constraint: Do NOT re-explore the codebase. Use the existing memories as your \
source of project knowledge. The goal is to update tool guidance and formatting, \
not to rediscover the project.

LAST ACTION: Call activate_project(\".\") to restore project state.

Return a brief summary of what changed in the system prompt."
    )
}
```

- [ ] **Step 4: Implement `build_prompt_refresh_main_instructions()`**

```rust
/// Build dispatch instructions for the main agent when a system prompt refresh is needed.
fn build_prompt_refresh_main_instructions(stored: Option<u32>, current: u32) -> String {
    let version_note = match stored {
        Some(v) => format!("v{v} → v{current}"),
        None => format!("unversioned → v{current}"),
    };
    format!(
        "\
System prompt outdated ({version_note}). The project's tool API guidance needs updating.

Spawn a general-purpose subagent with model=sonnet to refresh the system prompt. \
Pass the content of the `subagent_prompt` field as the subagent's task prompt. \
Do NOT read or summarize `subagent_prompt` yourself — pass it through as-is.

The subagent will:
1. Read existing project memories (no re-exploration)
2. Regenerate the system prompt with current tool guidance
3. Write the updated .codescout/system-prompt.md

Wait for the subagent to complete before continuing.

Do NOT attempt to refresh the system prompt yourself — the subagent handles it.

If the subagent fails, report the error to the user. They can re-run with \
onboarding(refresh_prompt=true) or onboarding(force=true) for a full re-onboarding.

If you cannot spawn subagents, execute the subagent prompt directly."
    )
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib prompt_refresh -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add prompt refresh subagent and instruction builders"
```

---

### Task 4: Wire up version check in `Onboarding::call` fast path

**Files:**
- Modify: `src/tools/workflow.rs` — `Onboarding::call` method

This is the core change: the fast path now checks the version and branches to a refresh response when stale.

- [ ] **Step 1: Write the failing test for stale version triggering refresh**

```rust
#[tokio::test]
async fn onboarding_triggers_refresh_when_version_stale() {
    let dir = tempdir().unwrap();
    // Create a project with config + onboarding memory but NO onboarding_version
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: None, // pre-versioning → stale
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    // Write the onboarding memory so it passes the has_config && has_memory gate
    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

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

    // Should return a refresh response, not the fast-path status
    assert!(
        result.get("subagent_prompt").is_some(),
        "stale version must trigger refresh with subagent_prompt"
    );
    assert_eq!(
        result["version_stale"].as_bool(),
        Some(true),
        "must indicate version is stale"
    );
    assert!(
        result["subagent_prompt"].as_str().unwrap().contains("Do NOT re-explore"),
        "subagent prompt must be the lightweight refresh, not full onboarding"
    );
}
```

- [ ] **Step 2: Write the test for current version returning normal fast path**

```rust
#[tokio::test]
async fn onboarding_fast_path_when_version_current() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: Some(ONBOARDING_VERSION), // current
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

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

    // Should return normal fast-path status
    assert_eq!(result["onboarded"].as_bool(), Some(true));
    assert!(
        result.get("subagent_prompt").is_none(),
        "current version must not trigger refresh"
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib onboarding_triggers_refresh -- --nocapture && cargo test --lib onboarding_fast_path_when_version -- --nocapture`
Expected: FAIL — version check not implemented, stale project returns fast-path status

- [ ] **Step 4: Modify the fast path in `Onboarding::call`**

In the fast path block (inside `if has_config && has_onboarding_memory`), after fetching memories, add the version check. The current code at this point has access to the project via `ctx.agent.with_project()`. You need to:

1. Read the stored `onboarding_version` from the config
2. Check `onboarding_version_stale(stored)`
3. If stale: build the refresh response instead of the fast-path status

The modification goes inside the `if has_config && has_onboarding_memory {` block, **before** the existing status message construction. Add:

```rust
                // Version check: refresh system prompt if stale
                // Read version and languages from config (in-memory clone from activation)
                let (stored_version, config_languages) = ctx
                    .agent
                    .with_project(|p| Ok((
                        p.config.project.onboarding_version,
                        p.config.project.languages.clone(),
                    )))
                    .await?;

                // Check for downgrade (log only, no action)
                if let Some(v) = stored_version {
                    if v > ONBOARDING_VERSION {
                        tracing::warn!(
                            "stored onboarding version ({}) is newer than compiled ({}) — skipping refresh. Run onboarding(force=true) if you downgraded intentionally.",
                            v,
                            ONBOARDING_VERSION
                        );
                    }
                }

                if onboarding_version_stale(stored_version) {
                    tracing::info!(
                        "onboarding version stale: stored={:?} current={}",
                        stored_version,
                        ONBOARDING_VERSION
                    );

                    // Optimistic version write — prevents infinite re-trigger
                    // Note: writes to disk only. The in-memory config (p.config) is
                    // a clone from activation and won't reflect this until reactivation.
                    // Within a single session, the refresh may re-trigger — this is
                    // acceptable (the subagent is lightweight).
                    ctx.agent
                        .with_project(|p| {
                            let config_path = p.root.join(".codescout").join("project.toml");
                            if config_path.exists() {
                                let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                                config.project.onboarding_version = Some(ONBOARDING_VERSION);
                                let toml_str = toml::to_string_pretty(&config)?;
                                std::fs::write(&config_path, &toml_str)?;
                            }
                            Ok(())
                        })
                        .await?;

                    let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);
                    let main_agent_instructions = build_prompt_refresh_main_instructions(
                        stored_version,
                        ONBOARDING_VERSION,
                    );

                    return Ok(json!({
                        "onboarded": true,
                        "version_stale": true,
                        "stored_version": stored_version,
                        "current_version": ONBOARDING_VERSION,
                        "languages": config_languages,
                        "config_created": false,
                        "subagent_prompt": subagent_prompt,
                        "main_agent_instructions": main_agent_instructions,
                    }));
                }
```

**Note on in-memory config:** `p.config` is a clone from activation time. The optimistic version write updates disk only — within the same session, the version check may re-trigger until reactivation. This is acceptable (the refresh subagent is lightweight).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib onboarding_triggers_refresh -- --nocapture && cargo test --lib onboarding_fast_path_when_version -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): wire version check into fast path with optimistic write"
```

---

### Task 5: Add `refresh_prompt` parameter and handling

**Files:**
- Modify: `src/tools/workflow.rs` — `input_schema` and `call` method

- [ ] **Step 1: Write the failing test for explicit refresh_prompt**

```rust
#[tokio::test]
async fn onboarding_refresh_prompt_returns_refresh_response() {
    // project_ctx() does NOT create an onboarded project — set up manually
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: Some(ONBOARDING_VERSION), // current version
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

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

    // Even with current version, refresh_prompt=true forces refresh
    let result = Onboarding.call(json!({"refresh_prompt": true}), &ctx).await.unwrap();

    assert!(
        result.get("subagent_prompt").is_some(),
        "refresh_prompt=true must return subagent_prompt"
    );
    assert!(
        result["subagent_prompt"].as_str().unwrap().contains("Do NOT re-explore"),
        "must be lightweight refresh, not full onboarding"
    );
}

#[tokio::test]
async fn onboarding_refresh_prompt_errors_when_not_onboarded() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    // No config, no memories — not onboarded
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
    let result = Onboarding.call(json!({"refresh_prompt": true}), &ctx).await.unwrap();
    // Should return an error (RecoverableError shows as JSON with "error" field)
    assert!(
        result.get("error").is_some(),
        "refresh_prompt on unonboarded project must return error"
    );
}

#[tokio::test]
async fn onboarding_force_takes_priority_over_refresh_prompt() {
    // project_ctx() creates an unonboarded project — force=true works regardless
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({"force": true, "refresh_prompt": true}), &ctx).await.unwrap();

    // force=true should trigger full onboarding (has Explore the Code), not lightweight refresh
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
        "force=true must trigger full onboarding, not lightweight refresh"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib onboarding_refresh_prompt -- --nocapture`
Expected: FAIL — `refresh_prompt` not parsed

- [ ] **Step 3: Add `refresh_prompt` to input_schema**

In `Onboarding`'s `input_schema()` method, add the parameter:

```rust
                    "refresh_prompt": {
                        "type": "boolean",
                        "description": "Regenerate system prompt from current templates without re-exploring (default: false)"
                    }
```

- [ ] **Step 4: Parse `refresh_prompt` in `call`**

At the top of the `call` method, after `let force = parse_bool_param(&input["force"]);`, add:

```rust
        let refresh_prompt = parse_bool_param(&input["refresh_prompt"]);
```

- [ ] **Step 5: Add refresh_prompt handling logic**

The logic goes **before** the `if !force {` block. If `refresh_prompt` is true and `force` is false:

```rust
        // refresh_prompt=true: lightweight system prompt refresh (force takes priority)
        if refresh_prompt && !force {
            let root = ctx.agent.require_project_root().await?;
            let status = ctx
                .agent
                .with_project(|p| {
                    let has_config = p.root.join(".codescout").join("project.toml").exists();
                    let memories = p.memory.list()?;
                    let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
                    Ok((has_config, has_onboarding_memory, memories))
                })
                .await?;
            let (has_config, has_onboarding_memory, memories) = status;

            if !has_config || !has_onboarding_memory {
                return Err(RecoverableError::with_hint(
                    "Project not yet onboarded — no memories to build a system prompt from.",
                    "Run onboarding() first to explore the codebase and create memories.",
                )
                .into());
            }

            // Read stored version BEFORE the optimistic write
            let stored_version = ctx
                .agent
                .with_project(|p| Ok(p.config.project.onboarding_version))
                .await?;

            // Optimistic version write
            ctx.agent
                .with_project(|p| {
                    let config_path = p.root.join(".codescout").join("project.toml");
                    if config_path.exists() {
                        let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                        config.project.onboarding_version = Some(ONBOARDING_VERSION);
                        let toml_str = toml::to_string_pretty(&config)?;
                        std::fs::write(&config_path, &toml_str)?;
                    }
                    Ok(())
                })
                .await?;

            let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);
            let main_agent_instructions = build_prompt_refresh_main_instructions(
                stored_version,
                ONBOARDING_VERSION,
            );

            return Ok(json!({
                "onboarded": true,
                "version_stale": false,
                "system_prompt_refresh": true,
                "current_version": ONBOARDING_VERSION,
                "subagent_prompt": subagent_prompt,
                "main_agent_instructions": main_agent_instructions,
            }));
        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib onboarding_refresh_prompt -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add refresh_prompt parameter for explicit system prompt refresh"
```

---

### Task 6: Update `call_content` routing discriminator

**Files:**
- Modify: `src/tools/workflow.rs` — `Onboarding::call_content` method

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_call_content_returns_two_blocks_for_version_refresh() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: None, // stale
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

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

    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
    assert_eq!(
        content.len(),
        2,
        "version refresh must return 2 content blocks"
    );
    let block1 = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(block1.contains("outdated") || block1.contains("subagent"));
    let block2 = content[1].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(block2.contains("--- ONBOARDING SUBAGENT PROMPT"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib onboarding_call_content_returns_two_blocks_for_version -- --nocapture`
Expected: FAIL — currently routes to single-block fast path because `onboarded: true`

- [ ] **Step 3: Update `call_content` routing**

Replace the routing logic in `call_content`. Change:

```rust
        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }
```

To:

```rust
        // Route based on presence of subagent_prompt, not just onboarded flag.
        // Version refresh sets onboarded=true AND subagent_prompt — needs two blocks.
        if val.get("subagent_prompt").is_some() {
            // Two-block response (full onboarding OR version refresh OR explicit refresh_prompt)
            let compact = format_onboarding(&val);
            let instructions = val["main_agent_instructions"].as_str().unwrap_or("");
            let block1 = format!("{}\n\n{}", compact, instructions);

            let subagent_prompt = val["subagent_prompt"].as_str().unwrap_or("");
            let block2 = format!(
                "--- ONBOARDING SUBAGENT PROMPT (pass as-is to subagent) ---\n\n{}",
                subagent_prompt
            );

            return Ok(vec![
                rmcp::model::Content::text(block1),
                rmcp::model::Content::text(block2),
            ]);
        }

        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }
```

This puts `subagent_prompt` check first — it catches all three subagent paths. The `onboarded` check only triggers for the genuine fast path (no subagent_prompt).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib onboarding_call_content -- --nocapture`
Expected: All call_content tests pass including the new one

- [ ] **Step 5: Run full test suite**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): update call_content routing to handle version refresh path"
```

---

### Task 7: Write version after full onboarding + add CLAUDE.md rule

**Files:**
- Modify: `src/tools/workflow.rs` — full onboarding response assembly
- Modify: `CLAUDE.md`

- [ ] **Step 1: Write the test for version write after full onboarding**

```rust
#[tokio::test]
async fn onboarding_full_writes_version_to_config() {
    let (_dir, ctx) = project_ctx().await;
    // force=true triggers full onboarding
    let _result = Onboarding.call(json!({"force": true}), &ctx).await.unwrap();

    // Check that the version was written to config
    let stored = ctx
        .agent
        .with_project(|p| {
            let config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
            Ok(config.project.onboarding_version)
        })
        .await
        .unwrap();
    assert_eq!(
        stored,
        Some(ONBOARDING_VERSION),
        "full onboarding must write ONBOARDING_VERSION to config"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib onboarding_full_writes_version -- --nocapture`
Expected: FAIL — version not written (still None)

- [ ] **Step 3: Add optimistic version write to full onboarding path**

In the full onboarding path in `Onboarding::call`, just before the final `Ok(json!({...}))` that returns the subagent_prompt response, add:

```rust
        // Optimistic version write — prevents re-trigger on next session
        ctx.agent
            .with_project(|p| {
                let config_path = p.root.join(".codescout").join("project.toml");
                if config_path.exists() {
                    let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                    config.project.onboarding_version = Some(ONBOARDING_VERSION);
                    let toml_str = toml::to_string_pretty(&config)?;
                    std::fs::write(&config_path, &toml_str)?;
                }
                Ok(())
            })
            .await?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib onboarding_full_writes_version -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add CLAUDE.md rule**

Add after the "Prompt Surface Consistency" section content (after line ~252 in CLAUDE.md):

```markdown
### Onboarding Version

When modifying system prompt surfaces, bump `ONBOARDING_VERSION` in
`src/tools/workflow.rs`. This triggers automatic system prompt refresh for all
projects onboarded with the previous version.

Bump when the generated system prompt would reference tool names, parameters,
or workflows that no longer exist:
- Tool names change (rename, consolidate)
- Tool parameter semantics change
- Server instructions (`server_instructions.md`) change significantly
- Onboarding prompt templates change in ways that affect the generated system prompt

Do NOT bump for:
- Bug fixes that don't change tool behavior
- Internal refactors
- Memory template changes (memories are re-read during refresh anyway)
```

- [ ] **Step 6: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: Clean

- [ ] **Step 7: Run full test suite**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/tools/workflow.rs CLAUDE.md
git commit -m "feat(onboarding): write version after full onboarding + add CLAUDE.md version bump rule"
```

---

### Task 8: Manual E2E verification

**Files:** None (testing only)

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: Build succeeds

- [ ] **Step 2: Restart MCP server**

Run `/mcp` in Claude Code to restart with the new binary.

- [ ] **Step 3: Test version refresh on existing project**

On the main `code-explorer` project (which has no `onboarding_version` in its config):

Run `onboarding()` — should detect stale version and return refresh response with `version_stale: true` and `subagent_prompt`.

- [ ] **Step 4: Test fast path after refresh**

Run `onboarding()` again — should return normal fast-path status (version now current).

- [ ] **Step 5: Test explicit `refresh_prompt=true`**

Run `onboarding(refresh_prompt=true)` — should return refresh response regardless of version.

- [ ] **Step 6: Test `force=true` still works**

Run `onboarding(force=true)` — should return full onboarding response (not lightweight refresh).

- [ ] **Step 7: Verify `project.toml` has version**

Check `.codescout/project.toml` — should contain `onboarding_version = 1`.
