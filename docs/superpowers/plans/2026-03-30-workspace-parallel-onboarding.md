# Workspace Parallel Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** For workspace onboarding, generate N per-project prompt files + 1 synthesis prompt, enabling parallel subagent dispatch instead of one giant sequential subagent.

**Architecture:** `call_content()` detects workspaces (>1 project) and writes tailored per-project .md files to `.codescout/tmp/`. Each file is self-contained (~15-20KB) with exploration steps scoped to that project. The synthesis prompt reads back per-project memories and writes workspace-level memories + updates CLAUDE.md. Main agent dispatches per-project subagents in parallel, then runs synthesis.

**Tech Stack:** Rust, `std::fs` for file writing, existing `GatheredContext` and `DiscoveredProject` types

---

### Task 1: Add `build_per_project_prompt` function

**Files:**
- Modify: `src/tools/workflow.rs` (add function near other prompt builders)

This function assembles a self-contained onboarding prompt for one project in a workspace.

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `src/tools/workflow.rs`:

```rust
#[test]
fn build_per_project_prompt_contains_project_context() {
    let project = crate::workspace::DiscoveredProject {
        id: "backend".to_string(),
        relative_root: std::path::PathBuf::from("."),
        languages: vec!["kotlin".to_string(), "java".to_string()],
        manifest: Some("build.gradle.kts".to_string()),
    };
    let siblings = vec![
        ("mcp-server".to_string(), vec!["rust".to_string()]),
        ("python-svc".to_string(), vec!["python".to_string()]),
    ];
    let prompt = build_per_project_prompt(&project, &siblings);

    // Must contain project identity
    assert!(prompt.contains("backend"), "must contain project id");
    assert!(prompt.contains("kotlin"), "must contain languages");
    assert!(prompt.contains("build.gradle.kts"), "must contain manifest");

    // Must contain sibling info (for context, not deep-diving)
    assert!(prompt.contains("mcp-server"), "must mention siblings");
    assert!(prompt.contains("Do NOT deep-dive"), "must warn against sibling deep-dives");

    // Must contain exploration steps
    assert!(prompt.contains("## Phase 2: Explore"), "must contain exploration phase");
    assert!(prompt.contains("list_symbols"), "must contain exploration instructions");

    // Must contain memory writing instructions
    assert!(prompt.contains("## Phase 3: Write"), "must contain memory phase");
    assert!(prompt.contains("project-overview"), "must reference memory topics");

    // Must contain iron law
    assert!(prompt.contains("IRON LAW"), "must contain iron law");

    // Must contain return contract
    assert!(prompt.contains("## Return Contract"), "must contain return contract");

    // Must NOT contain workspace synthesis instructions
    assert!(!prompt.contains("Workspace Memory Synthesis"), "must NOT contain workspace synthesis");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_per_project_prompt_contains_project_context`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement `build_per_project_prompt`**

Add near the other prompt builder functions (~line 935 area):

```rust
/// Build a self-contained onboarding prompt for one project in a workspace.
///
/// Each per-project subagent gets this prompt. It includes:
/// - Iron Law, exploration steps, red flags (from the shared template)
/// - Project-specific context (id, root, languages, siblings)
/// - Per-project memory writing instructions (scoped with project= param)
/// - Return contract
fn build_per_project_prompt(
    project: &crate::workspace::DiscoveredProject,
    siblings: &[(String, Vec<String>)],
) -> String {
    let mut prompt = String::new();

    // Iron Law (from shared template)
    prompt.push_str("## THE IRON LAW\n\n");
    prompt.push_str("```\nNO MEMORIES WRITTEN WITHOUT COMPLETING ALL EXPLORATION STEPS FIRST\n```\n\n");
    prompt.push_str("You may only call `memory(action: \"write\", ...)` after you have:\n");
    prompt.push_str("1. Completed ALL exploration steps below\n");
    prompt.push_str("2. Verified EVERY item in the Phase 2 Gate Checklist\n\n");
    prompt.push_str("---\n\n");

    // Project context
    prompt.push_str("## Your Project\n\n");
    prompt.push_str(&format!("- **ID:** {}\n", project.id));
    prompt.push_str(&format!("- **Root:** {}\n", project.relative_root.display()));
    prompt.push_str(&format!("- **Languages:** {}\n", project.languages.join(", ")));
    if let Some(ref manifest) = project.manifest {
        prompt.push_str(&format!("- **Manifest:** {}\n", manifest));
    }

    if !siblings.is_empty() {
        prompt.push_str("\n**Sibling projects** (for context — Do NOT deep-dive these):\n");
        for (id, langs) in siblings {
            prompt.push_str(&format!("- {} ({})\n", id, langs.join(", ")));
        }
    }
    prompt.push_str("\n---\n\n");

    // Phase 2: Explore (scoped to this project)
    prompt.push_str("## Phase 2: Explore the Code\n\n");
    prompt.push_str("Explore ONLY your project root. Do NOT explore sibling projects.\n\n");
    prompt.push_str(&format!(
        "### Step 1: Map the Codebase Structure\n\n\
         - `list_dir(\"{root}\")` — top-level structure\n\
         - `list_dir` on each subdirectory\n\
         - `read_file` on the build config\n\
         - `read_markdown(\"README.md\")` if present\n\n",
        root = project.relative_root.display()
    ));
    prompt.push_str(
        "### Step 2: Full Symbol Survey\n\n\
         - Run `list_symbols` on the main source directory\n\
         - Run `list_symbols` on EACH subdirectory individually\n\
         - Survey at least 5 distinct source files\n\n"
    );
    prompt.push_str(
        "### Step 3: Read Core Implementations\n\n\
         - Identify 5+ central types/functions from Step 2\n\
         - `find_symbol(name, include_body=true)` for each\n\
         - Read the FULL body, not just signatures\n\n"
    );
    prompt.push_str(
        "### Step 4: Read Architecture Documentation\n\n\
         - `read_markdown` on any docs found in the project\n\
         - Read completely — do not skim\n\n"
    );
    prompt.push_str(
        "### Step 5: Trace Two Data Flows\n\n\
         - Trace the most representative operation end-to-end\n\
         - Trace a second distinct path (error, write vs read, etc.)\n\n"
    );
    prompt.push_str(
        "### Step 6: Concept-Level Search (5+ queries)\n\n\
         - Error handling, data flow, testing, config, domain concept\n\
         - Use `semantic_search` or `grep` as fallback\n\n"
    );
    prompt.push_str(
        "### Step 7: Examine Tests\n\n\
         - `list_symbols` on test directory\n\
         - Read 2-3 test files for patterns\n\n"
    );

    // Phase 2 Gate Checklist
    prompt.push_str(
        "### Phase 2 Gate Checklist\n\n\
         Before writing ANY memory, verify ALL true:\n\
         - [ ] Listed structure AND ran list_dir on major subdirectories\n\
         - [ ] Symbol survey on 5+ source files\n\
         - [ ] Read full body of 5+ core implementations\n\
         - [ ] Read all architecture docs\n\
         - [ ] Traced two data flows\n\
         - [ ] Ran 5+ concept queries\n\
         - [ ] Read 2-3 test files\n\n\
         ---\n\n"
    );

    // Red Flags
    prompt.push_str(
        "## Red Flags — STOP and Return to Phase 2\n\n\
         If you notice any of these, STOP and go back:\n\
         - \"I have a good enough picture\" — No, read the code.\n\
         - \"The README covers this\" — READMEs lie. Verify in code.\n\
         - \"This is similar to...\" — Explore anyway. Differences matter.\n\n\
         ---\n\n"
    );

    // Phase 3: Write per-project memories
    prompt.push_str("## Phase 3: Write the Memories\n\n");
    prompt.push_str(&format!(
        "Write these memories using `memory(action=\"write\", project=\"{id}\", topic=\"...\", content=\"...\")`.\n\n",
        id = project.id
    ));
    prompt.push_str(
        "### 1. `project-overview`\n\
         Purpose, tech stack, key dependencies, runtime requirements. 15-30 lines.\n\n\
         ### 2. `architecture`\n\
         Module structure, key abstractions, data flow, design patterns. 20-40 lines.\n\
         Include 3-5 good `semantic_search(query, project=\"{id}\")` examples.\n\n\
         ### 3. `conventions`\n\
         Language/framework patterns, naming, testing approach. 15-30 lines.\n\n"
    );

    // Return contract
    prompt.push_str("---\n\n");
    prompt.push_str(
        "## Return Contract\n\n\
         Return a summary with:\n\
         - What this project does (your own words)\n\
         - 3-5 most important types/modules\n\
         - How a typical operation flows\n\
         - Memories written (list topics)\n\
         - Any issues encountered\n"
    );

    prompt
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test build_per_project_prompt_contains_project_context`
Expected: PASS

- [ ] **Step 5: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add build_per_project_prompt for workspace parallel dispatch"
```

---

### Task 2: Add `build_synthesis_prompt` function

**Files:**
- Modify: `src/tools/workflow.rs` (add function)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn build_synthesis_prompt_contains_readback_and_claude_md() {
    let projects = vec![
        ("backend".to_string(), vec!["kotlin".to_string()]),
        ("mcp-server".to_string(), vec!["rust".to_string()]),
    ];
    let prompt = build_synthesis_prompt(&projects);

    // Must contain memory readback commands for each project
    assert!(prompt.contains("memory(action=\"read\", project=\"backend\""));
    assert!(prompt.contains("memory(action=\"read\", project=\"mcp-server\""));

    // Must contain workspace memory topics
    assert!(prompt.contains("architecture"));
    assert!(prompt.contains("conventions"));
    assert!(prompt.contains("development-commands"));
    assert!(prompt.contains("domain-glossary"));
    assert!(prompt.contains("gotchas"));

    // Must contain CLAUDE.md refresh instructions
    assert!(prompt.contains("CLAUDE.md"), "must include CLAUDE.md refresh");
    assert!(prompt.contains("preserve"), "must mention preserving user content");

    // Must contain system prompt generation
    assert!(prompt.contains("system-prompt"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_synthesis_prompt_contains_readback_and_claude_md`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement `build_synthesis_prompt`**

```rust
/// Build the workspace synthesis prompt that runs after all per-project subagents complete.
///
/// The main agent (or a synthesis subagent) reads this to:
/// 1. Read back all per-project memories
/// 2. Write 5 workspace-level memories
/// 3. Generate the system prompt
/// 4. Offer to refresh CLAUDE.md with memory references
fn build_synthesis_prompt(projects: &[(String, Vec<String>)]) -> String {
    let mut prompt = String::new();

    // Step 1: Read back per-project memories
    prompt.push_str("## Read Per-Project Memories\n\n");
    prompt.push_str("Read these memories to understand what each subagent discovered:\n\n");
    for (id, _langs) in projects {
        prompt.push_str(&format!(
            "- `memory(action=\"read\", project=\"{id}\", topic=\"project-overview\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"architecture\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"conventions\")`\n"
        ));
    }
    prompt.push_str("\n---\n\n");

    // Step 2: Write workspace-level memories
    prompt.push_str("## Write Workspace Memories\n\n");
    prompt.push_str(
        "Write these 5 workspace-level memories (no `project:` parameter = workspace-level):\n\n"
    );
    prompt.push_str(
        "### 1. `architecture`\n\
         Workspace-level architecture:\n\
         - Project map: each project's purpose (1 sentence each)\n\
         - Cross-project dependencies (which imports from which)\n\
         - Shared infrastructure (CI, deployment, tooling)\n\
         15-30 lines.\n\n\
         ### 2. `conventions`\n\
         Shared patterns across projects: commit style, PR process, CI rules.\n\
         Per-project: reference `memory(project=\"{id}\", topic=\"conventions\")`.\n\
         15-30 lines.\n\n\
         ### 3. `development-commands`\n\
         Workspace-level build/test/lint commands. Per-project commands go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 4. `domain-glossary`\n\
         Terms used across multiple projects. Project-specific terms go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 5. `gotchas`\n\
         Cross-project pitfalls, version mismatches, integration gotchas.\n\
         10-20 lines.\n\n"
    );

    // Step 3: System prompt
    prompt.push_str("---\n\n## Generate System Prompt\n\n");
    prompt.push_str(
        "Write `system-prompt.md` using `memory(action=\"write\", topic=\"system-prompt\", content=\"...\")`.\n\
         Include: entry points per project, key abstractions, search tips scoped by project,\n\
         navigation strategy for the workspace.\n\n"
    );

    // Step 4: CLAUDE.md refresh
    prompt.push_str("---\n\n## Refresh CLAUDE.md\n\n");
    prompt.push_str(
        "Read `read_markdown(\"CLAUDE.md\")` to see its heading structure.\n\n\
         Compare each section with the memories you just wrote. For sections that\n\
         overlap with memory content, offer to replace the body with a memory reference:\n\
         `See codescout memory 'architecture' (Key Patterns section).`\n\n\
         **Preserve user-specific content:** personal preferences, code style rules,\n\
         iron rules, git workflow specifics, private notes — anything not derivable\n\
         from the codebase. Do NOT touch sections the user wrote for their own use.\n\n\
         **Add memory discovery hints** if CLAUDE.md doesn't already list available memories.\n\n\
         Present a summary of proposed changes and ask for approval before modifying.\n\n"
    );

    // Return contract
    prompt.push_str("---\n\n## Return Contract\n\n");
    prompt.push_str(
        "Return a summary with:\n\
         - Workspace-level memories written (list topics)\n\
         - Cross-project patterns discovered\n\
         - CLAUDE.md changes proposed/applied\n\
         - Any issues or gaps\n"
    );

    prompt
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test build_synthesis_prompt_contains_readback_and_claude_md`
Expected: PASS

- [ ] **Step 5: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add build_synthesis_prompt for workspace memory synthesis"
```

---

### Task 3: Add `build_workspace_instructions` function

**Files:**
- Modify: `src/tools/workflow.rs` (add function)

This builds the dispatch instructions for workspace mode, referencing the per-project and synthesis prompt file paths.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn build_workspace_instructions_claude_contains_parallel_dispatch() {
    let project_prompts = vec![
        ("backend".to_string(), ".codescout/tmp/onboarding-project-backend.md".to_string()),
        ("mcp".to_string(), ".codescout/tmp/onboarding-project-mcp.md".to_string()),
    ];
    let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
    let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
    let instructions = build_workspace_instructions(
        main_prompt_path, &project_prompts, synthesis_path, true,
    );

    // Must mention parallel dispatch
    assert!(instructions.contains("parallel") || instructions.contains("PARALLEL"));
    // Must reference each project prompt
    assert!(instructions.contains("onboarding-project-backend.md"));
    assert!(instructions.contains("onboarding-project-mcp.md"));
    // Must reference synthesis prompt
    assert!(instructions.contains("onboarding-workspace-synthesis.md"));
    // Must reference Phase 0-1 from main prompt
    assert!(instructions.contains("Phase 0") || instructions.contains("Phase 1"));
    // Must mention subagent
    assert!(instructions.contains("subagent"));
}

#[test]
fn build_workspace_instructions_generic_is_sequential() {
    let project_prompts = vec![
        ("backend".to_string(), ".codescout/tmp/onboarding-project-backend.md".to_string()),
    ];
    let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
    let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
    let instructions = build_workspace_instructions(
        main_prompt_path, &project_prompts, synthesis_path, false,
    );

    assert!(!instructions.contains("subagent"));
    assert!(instructions.contains("onboarding-project-backend.md"));
    assert!(instructions.contains("read_markdown"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_workspace_instructions`
Expected: FAIL

- [ ] **Step 3: Implement `build_workspace_instructions`**

```rust
/// Build dispatch instructions for workspace onboarding.
fn build_workspace_instructions(
    main_prompt_path: &str,
    project_prompts: &[(String, String)],
    synthesis_path: &str,
    subagent_capable: bool,
) -> String {
    let p = main_prompt_path;

    if subagent_capable {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites from the main prompt:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Spawn {} subagents IN PARALLEL — one per project:",
            project_prompts.len(),
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Wait for ALL subagents to complete.\n\n\
Step 4: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")\n\n\
Follow the synthesis instructions to read back per-project memories,\n\
write workspace-level memories, generate the system prompt, and\n\
offer to refresh CLAUDE.md."
        ));

        instructions
    } else {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Explore each project one at a time:",
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")"
        ));

        instructions
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test build_workspace_instructions && cargo fmt && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add build_workspace_instructions for parallel dispatch"
```

---

### Task 4: Update `call_content` for workspace dispatch

**Files:**
- Modify: `src/tools/workflow.rs` (`Onboarding::call_content`)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_call_content_workspace_writes_per_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create a workspace with 2 projects
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(root.join("sub-project/src")).unwrap();
    std::fs::write(root.join("sub-project/Cargo.toml"), "[package]\nname = \"sub\"").unwrap();
    std::fs::write(root.join("sub-project/src/lib.rs"), "pub fn hello() {}").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
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

    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");

    // Must have project_prompts array
    let project_prompts = parsed["project_prompts"].as_array()
        .expect("workspace must have project_prompts");
    assert!(project_prompts.len() >= 2, "must have at least 2 project prompts");

    // Each entry must have id and path
    for pp in project_prompts {
        let id = pp["id"].as_str().expect("must have id");
        let path = pp["path"].as_str().expect("must have path");
        assert!(path.contains("onboarding-project-"), "path must contain project prefix");
        // File must exist
        assert!(root.join(path).exists(), "prompt file must exist for {}", id);
    }

    // Must have synthesis_prompt_path
    let synthesis_path = parsed["synthesis_prompt_path"].as_str()
        .expect("must have synthesis_prompt_path");
    assert!(root.join(synthesis_path).exists(), "synthesis file must exist");

    // Instructions must mention parallel or sequential
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_call_content_workspace_writes_per_project_files`
Expected: FAIL

- [ ] **Step 3: Update `call_content`**

In the existing `call_content`, after writing the main prompt file and building the heading map, add workspace detection:

```rust
// After writing the main prompt file and building sections/instructions...

// For workspaces, also write per-project and synthesis prompt files.
let workspace_fields = if val["workspace_mode"].as_bool().unwrap_or(false) {
    let projects_val = val["projects"].as_array();
    if let Some(projects) = projects_val {
        let mut project_prompts = Vec::new();
        let all_projects: Vec<(String, Vec<String>)> = projects.iter().filter_map(|p| {
            let id = p["id"].as_str()?.to_string();
            let langs: Vec<String> = p["languages"].as_array()?
                .iter().filter_map(|l| l.as_str().map(String::from)).collect();
            Some((id, langs))
        }).collect();

        for p in projects {
            let id = p["id"].as_str().unwrap_or("unknown");
            let project = crate::workspace::DiscoveredProject {
                id: id.to_string(),
                relative_root: std::path::PathBuf::from(
                    p["root"].as_str().unwrap_or(".")
                ),
                languages: p["languages"].as_array()
                    .map(|a| a.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                manifest: p["manifest"].as_str().map(String::from),
            };
            let siblings: Vec<(String, Vec<String>)> = all_projects.iter()
                .filter(|(sid, _)| sid != id)
                .cloned()
                .collect();

            let prompt_content = build_per_project_prompt(&project, &siblings);
            let file_name = format!("onboarding-project-{}.md", id);
            let file_path = tmp_dir.join(&file_name);
            std::fs::write(&file_path, &prompt_content)?;

            let rel = format!(".codescout/tmp/{}", file_name);
            project_prompts.push((id.to_string(), rel));
        }

        // Write synthesis prompt
        let synthesis_content = build_synthesis_prompt(&all_projects);
        let synthesis_file = tmp_dir.join("onboarding-workspace-synthesis.md");
        std::fs::write(&synthesis_file, &synthesis_content)?;
        let synthesis_rel = ".codescout/tmp/onboarding-workspace-synthesis.md".to_string();

        // Build workspace-specific instructions (overrides the single-project ones)
        let ws_instructions = build_workspace_instructions(
            rel_path, &project_prompts, &synthesis_rel, subagent,
        );

        Some((project_prompts, synthesis_rel, ws_instructions))
    } else {
        None
    }
} else {
    None
};

// Build the response — workspace fields override instructions if present
let response = if let Some((project_prompts, synthesis_path, ws_instructions)) = workspace_fields {
    let pp_json: Vec<Value> = project_prompts.iter().map(|(id, path)| {
        serde_json::json!({ "id": id, "path": path })
    }).collect();

    serde_json::json!({
        "prompt_path": rel_path,
        "summary": compact,
        "sections": sections,
        "project_prompts": pp_json,
        "synthesis_prompt_path": synthesis_path,
        "instructions": ws_instructions,
    })
} else {
    serde_json::json!({
        "prompt_path": rel_path,
        "summary": compact,
        "sections": sections,
        "instructions": instructions,
    })
};
```

Note: This replaces the current response-building code at the end of the `if let Some(prompt)` block. The single-project path stays the same.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test onboarding_call_content_workspace_writes_per_project_files`
Expected: PASS

- [ ] **Step 5: Run full tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): workspace call_content writes per-project + synthesis files"
```

---

### Task 5: Update existing workspace tests

**Files:**
- Modify: `src/tools/workflow.rs` (test functions)

- [ ] **Step 1: Find and update workspace-related tests**

Search for tests containing "workspace" in their name:
- `onboarding_call_content_includes_workspace_info`
- `workspace_onboarding_full_flow`
- `single_project_onboarding_unchanged`
- `onboarding_includes_workspace_mode_and_per_project_protected`

Update each to check for `project_prompts` and `synthesis_prompt_path` in workspace responses.

For `single_project_onboarding_unchanged`: verify that `project_prompts` is NOT present.

For workspace tests: verify `project_prompts` array, `synthesis_prompt_path`, and that per-project files exist on disk.

- [ ] **Step 2: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "test(onboarding): update workspace tests for parallel dispatch response"
```

---

### Task 6: Add CLAUDE.md refresh instructions to single-project "After Everything"

**Files:**
- Modify: `src/prompts/onboarding_prompt.md` (## After Everything Is Created section)

- [ ] **Step 1: Read current "After Everything" section**

Use `read_markdown("src/prompts/onboarding_prompt.md", heading="## After Everything Is Created")` to see current content.

- [ ] **Step 2: Add CLAUDE.md refresh instructions**

Append to the "After Everything Is Created" section:

```markdown
### Refresh CLAUDE.md

Read `read_markdown("CLAUDE.md")` to see its heading structure.

Compare each section with the memories you just wrote. For sections that
overlap with memory content, offer to replace the body with a memory reference:
`See codescout memory 'architecture' (Key Patterns section).`

**Preserve user-specific content:** personal preferences, code style rules,
iron rules, git workflow specifics, private notes — anything not derivable
from the codebase. Do NOT touch sections the user wrote for their own use.

**Add memory discovery hints** if CLAUDE.md doesn't already list available
memory topics so future agents know they exist.

Present a summary of proposed changes and ask for approval before modifying.
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs(onboarding): add CLAUDE.md refresh instructions to After Everything section"
```

---

### Task 7: Build release and verify via MCP

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`

- [ ] **Step 2: Restart MCP server**

Run `/mcp` in Claude Code.

- [ ] **Step 3: Verify single-project onboarding**

Call `onboarding(force: true)` on a single-project repo. Verify:
- No `project_prompts` or `synthesis_prompt_path` in response
- Same behavior as before

- [ ] **Step 4: Verify workspace onboarding**

Call `onboarding(force: true)` on a workspace. Verify:
- `project_prompts` array with N entries
- `synthesis_prompt_path` present
- Each per-project .md file exists and contains project-specific content
- Synthesis .md file exists and contains CLAUDE.md refresh instructions
- Instructions mention parallel subagent dispatch (for Claude Code)
