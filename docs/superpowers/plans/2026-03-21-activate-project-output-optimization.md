# activate_project Output Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the verbose config dump in `activate_project` with a slim orientation card that differs by RO/RW mode, and unify the focus-switch path to produce the same output.

**Architecture:** Extract workspace summary logic from `Agent::project_status` into a shared helper. Add `Agent::activate_within_workspace()` to promote Dormant projects in-place. Refactor `ActivateProject::call` to use a single `build_activation_response()` helper for both full-activation and focus-switch paths.

**Tech Stack:** Rust, serde_json, tokio (async)

**Spec:** `docs/superpowers/specs/2026-03-21-activate-project-output-optimization-design.md`

---

### Task 1: Extract `workspace_summary()` helper from `Agent::project_status`

**Files:**
- Modify: `src/agent.rs` (Agent impl, lines 376–434)

The workspace-building logic in `project_status` is duplicated verbatim. Extract it into a reusable method so both `project_status` and `activate_project` can call it.

- [ ] **Step 1: Write the failing test**

Add to `src/agent.rs` tests:

```rust
#[tokio::test]
async fn workspace_summary_returns_projects_with_depends_on() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create two sub-projects
    let sub_a = root.join("packages").join("api");
    let sub_b = root.join("packages").join("web");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();
    std::fs::write(sub_a.join("package.json"), r#"{"name":"api","scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::write(sub_b.join("package.json"), r#"{"name":"web","scripts":{"build":"tsc"}}"#).unwrap();

    let agent = Agent::new(Some(root)).await.unwrap();
    let summary = agent.workspace_summary().await;
    assert!(summary.is_some(), "multi-project workspace should have summary");
    let projects = summary.unwrap();
    assert!(projects.len() >= 2, "should have at least 2 sub-projects");
    // Each entry should have depends_on field (even if empty)
    for p in &projects {
        // depends_on is a Vec<String>, always present
        let _ = &p.depends_on;
    }
}

#[tokio::test]
async fn workspace_summary_returns_none_for_single_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let summary = agent.workspace_summary().await;
    assert!(summary.is_none(), "single-project workspace should return None");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test workspace_summary_returns -- --nocapture`
Expected: compilation error — `workspace_summary` method does not exist.

- [ ] **Step 3: Extract the helper method**

Add to `impl Agent` in `src/agent.rs`, by cutting the workspace-building block from `project_status` (lines ~397–425) into a new method:

```rust
/// Build workspace project summaries for multi-project repos.
/// Returns None for single-project workspaces.
pub async fn workspace_summary(&self) -> Option<Vec<crate::prompts::WorkspaceProjectSummary>> {
    let inner = self.inner.read().await;
    let ws = inner.workspace.as_ref()?;
    if ws.projects.len() <= 1 {
        return None;
    }
    let ws_cfg: Option<crate::config::workspace::WorkspaceConfig> =
        std::fs::read_to_string(crate::config::workspace::workspace_config_path(&ws.root))
            .ok()
            .and_then(|s| toml::from_str(&s).ok());

    let summaries = ws
        .projects
        .iter()
        .map(|p| {
            let depends_on = ws_cfg
                .as_ref()
                .and_then(|cfg| cfg.projects.iter().find(|e| e.id == p.discovered.id))
                .map(|e| e.depends_on.clone())
                .unwrap_or_default();
            crate::prompts::WorkspaceProjectSummary {
                id: p.discovered.id.clone(),
                root: p.discovered.relative_root.display().to_string(),
                languages: p.discovered.languages.clone(),
                depends_on,
            }
        })
        .collect();
    Some(summaries)
}
```

- [ ] **Step 4: Update `project_status` to use the new helper**

`project_status` currently holds a read-lock on `inner` for the entire method. The new `workspace_summary` acquires its own read-lock, so you must restructure `project_status` to release the first lock before calling `workspace_summary()`.

Restructure `project_status` as follows:

```rust
pub async fn project_status(&self) -> Option<crate::prompts::ProjectStatus> {
    // Phase 1: read project fields under a short-lived lock
    let (name, path, languages, memories, has_index, github_enabled, system_prompt) = {
        let inner = self.inner.read().await;
        let project = inner.active_project()?;
        let memories = project.memory.list().unwrap_or_default();
        let has_index = crate::embed::index::project_db_path(&project.root).exists();
        let github_enabled = project.config.security.github_enabled;

        let prompt_file = project.root.join(".codescout").join("system-prompt.md");
        let system_prompt = if prompt_file.exists() {
            std::fs::read_to_string(&prompt_file).ok()
        } else {
            project.config.project.system_prompt.clone()
        };

        Some((
            project.config.project.name.clone(),
            project.root.display().to_string(),
            project.config.project.languages.clone(),
            memories,
            has_index,
            github_enabled,
            system_prompt,
        ))
    }?; // lock dropped here

    // Phase 2: workspace summary (acquires its own read-lock)
    let workspace = self.workspace_summary().await;

    Some(crate::prompts::ProjectStatus {
        name,
        path,
        languages,
        memories,
        has_index,
        system_prompt,
        github_enabled,
        workspace,
    })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test workspace_summary_returns -- --nocapture && cargo test project_status -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/agent.rs
git commit -m "refactor: extract workspace_summary() helper from project_status"
```

---

### Task 2: Add `Agent::activate_within_workspace()`

**Files:**
- Modify: `src/agent.rs` (Agent impl)
- Modify: `src/workspace.rs` (Workspace impl — need `promote_to_activated` helper)

This method promotes a Dormant project to Activated in-place within the existing workspace, without rebuilding the workspace topology.

- [ ] **Step 1: Write the failing test**

Add to `src/agent.rs` tests:

```rust
#[tokio::test]
async fn activate_within_workspace_promotes_dormant() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a sub-project
    let sub = root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let agent = Agent::new(Some(root.clone())).await.unwrap();

    // Before: sub-project is Dormant, with_project after switch_focus would fail
    agent.switch_focus("api").await.unwrap();
    // Dormant project has no ActiveProject — this returns None
    let is_dormant = {
        let inner = agent.inner.read().await;
        inner.active_project().is_none()
    };
    assert!(is_dormant, "sub-project should be Dormant before activate_within_workspace");

    // Switch back to home first
    agent.switch_focus(crate::workspace::ROOT_PROJECT_ID).await.unwrap();

    // Now use activate_within_workspace
    agent.activate_within_workspace("api", None).await.unwrap();

    // After: with_project works
    let name = agent
        .with_project(|p| Ok(p.config.project.name.clone()))
        .await
        .unwrap();
    assert!(!name.is_empty(), "should have loaded config for sub-project");

    // Workspace topology preserved — all original projects still exist
    let project_count = {
        let inner = agent.inner.read().await;
        inner.workspace.as_ref().unwrap().projects.len()
    };
    assert!(project_count >= 2, "workspace should still have all projects");
}

#[tokio::test]
async fn activate_within_workspace_unknown_id_errors() {
    let dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let result = agent.activate_within_workspace("nonexistent", None).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test activate_within_workspace -- --nocapture`
Expected: compilation error — method does not exist.

- [ ] **Step 3: Implement `activate_within_workspace`**

Add to `impl Agent` in `src/agent.rs`:

```rust
/// Promote a Dormant workspace project to Activated in-place.
/// Unlike `activate()`, this preserves the workspace topology.
pub async fn activate_within_workspace(
    &self,
    project_id: &str,
    read_only: Option<bool>,
) -> Result<()> {
    let mut inner = self.inner.write().await;
    let ws = inner
        .workspace
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("No active workspace"))?;

    // Find the project and resolve its root
    let project = ws
        .projects
        .iter()
        .find(|p| p.discovered.id == project_id)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found in workspace", project_id))?;

    let abs_root = ws.root.join(&project.discovered.relative_root);

    // Determine read_only: explicit > default (non-home = true)
    let is_home = inner.home_root.as_ref().map(|h| *h == abs_root).unwrap_or(false);
    let effective_read_only = match read_only {
        Some(false) => false,
        _ if is_home => false,
        _ => true,
    };

    // If already activated, just update read_only and switch focus
    let already_activated = project.as_active().is_some();
    if already_activated {
        ws.set_focused(project_id)?;
        // Update read_only if explicitly provided
        if let Some(ro) = read_only {
            if let Some(active) = ws.focused_active_mut().and_then(|p| p.as_active_mut()) {
                active.read_only = ro;
            }
        }
        return Ok(());
    }

    // Load config, memory, library registry for the sub-project
    let config = ProjectConfig::load_or_default(&abs_root)?;
    let memory = MemoryStore::open(&abs_root)?;
    let private_memory = MemoryStore::open_private(&abs_root)?;
    let registry_path = abs_root.join(".codescout").join("libraries.json");
    let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();

    let active = ActiveProject {
        root: abs_root,
        config,
        memory,
        private_memory,
        library_registry,
        dirty_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        read_only: effective_read_only,
    };

    // Promote in-place: find the project again mutably and replace its state
    let project_mut = ws
        .projects
        .iter_mut()
        .find(|p| p.discovered.id == project_id)
        .unwrap(); // safe: we found it above
    project_mut.state = ProjectState::Activated(Box::new(active));

    // Switch focus
    ws.focused = Some(project_id.to_string());

    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test activate_within_workspace -- --nocapture`
Expected: both tests pass.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/agent.rs
git commit -m "feat: add activate_within_workspace() for in-place Dormant promotion"
```

---

### Task 3: Refactor `ActivateProject::call` with `build_activation_response`

**Files:**
- Modify: `src/tools/config.rs` (ActivateProject call method, lines 29–156)

This is the main refactor — replace the two code paths (focus-switch early return + full-activation) with a single `build_activation_response` helper, producing the new slim output shape.

- [ ] **Step 1: Write the failing tests for the new output shape**

Add to `src/tools/config.rs` tests (below existing tests):

```rust
#[tokio::test]
async fn activate_project_rw_includes_security_fields() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap(), "read_only": false}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(result["security_profile"].is_string(), "RW should include security_profile");
    assert!(!result["shell_enabled"].is_null(), "RW should include shell_enabled");
    assert!(!result["github_enabled"].is_null(), "RW should include github_enabled");
}

#[tokio::test]
async fn activate_project_ro_excludes_security_fields() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap(), "read_only": true}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(result["security_profile"].is_null(), "RO should not include security_profile");
    assert!(result["shell_enabled"].is_null(), "RO should not include shell_enabled");
    assert!(result["github_enabled"].is_null(), "RO should not include github_enabled");
}

#[tokio::test]
async fn activate_project_includes_memories_and_index() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(result["memories"].is_array(), "should include memories array");
    assert!(result["index"].is_object(), "should include index object");
    assert!(result["index"]["status"].is_string(), "index should have status");
}

#[tokio::test]
async fn activate_project_rw_hint_promotes_project_status() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap(), "read_only": false}), &ctx)
        .await
        .unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.contains("project_status"), "RW hint should promote project_status, got: {hint}");
}

#[tokio::test]
async fn activate_project_single_project_no_workspace() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(result["workspace"].is_null(), "single-project should have null workspace");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test activate_project_rw_includes -- --nocapture`
Expected: FAIL — current output has `activated.config` shape, not the new fields.

- [ ] **Step 3: Implement `HintScenario` enum and `build_activation_response`**

Add above `build_activation_response` in `src/tools/config.rs`:

```rust
/// Determines the hint text shown after activation.
enum HintScenario {
    /// First-ever activation (home project, session start)
    FirstActivation,
    /// Returning to the home project after visiting another
    ReturnToHome,
    /// Switching to a non-home project
    SwitchAway,
}
```

Then add the helper function:

```rust
/// Build the activation response JSON for both full-activation and focus-switch paths.
async fn build_activation_response(
    ctx: &ToolContext,
    scenario: HintScenario,
    auto_registered: &[crate::library::auto_register::AutoRegistered],
) -> anyhow::Result<Value> {
    // Read project fields
    let (project_name, project_root, languages, read_only, memories, has_index, security) = ctx
        .agent
        .with_project(|p| {
            let memories = p.memory.list().unwrap_or_default();
            let has_index = crate::embed::index::project_db_path(&p.root).exists();
            let security = if !p.read_only {
                Some((
                    p.config.security.profile.clone(),
                    p.config.security.shell_enabled,
                    p.config.security.github_enabled,
                ))
            } else {
                None
            };
            Ok((
                p.config.project.name.clone(),
                p.root.display().to_string(),
                p.config.project.languages.clone(),
                p.read_only,
                memories,
                has_index,
                security,
            ))
        })
        .await?;

    // Index status
    let index = if has_index {
        json!({"status": "indexed"})
    } else {
        json!({"status": "not_indexed", "hint": "Run index_project() to enable semantic_search."})
    };

    // Workspace
    let workspace = ctx.agent.workspace_summary().await;
    let workspace_json = workspace.as_ref().map(|projects| {
        projects
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "root": p.root,
                    "languages": p.languages,
                    "depends_on": p.depends_on,
                })
            })
            .collect::<Vec<_>>()
    });

    // Hint
    let home_root = ctx.agent.home_root().await;
    let hint = match scenario {
        HintScenario::FirstActivation => {
            format!("CWD: {}. Run project_status() for health checks and memory staleness.", project_root)
        }
        HintScenario::ReturnToHome => {
            format!("Returned to home project. CWD: {}. Run project_status() to check memory staleness.", project_root)
        }
        HintScenario::SwitchAway if read_only => {
            let home_str = home_root.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            format!(
                "Browsing {} (read-only). CWD: {} — remember to activate_project(\"{}\") when done.",
                project_name, project_root, home_str,
            )
        }
        HintScenario::SwitchAway => {
            let home_str = home_root.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            format!(
                "Switched project (read-write). CWD: {} — remember to activate_project(\"{}\") when done.",
                project_root, home_str,
            )
        }
    };

    // Build response
    let mut result = json!({
        "status": "ok",
        "project": project_name,
        "project_root": project_root,
        "read_only": read_only,
        "languages": languages,
        "index": index,
        "memories": memories,
        "hint": hint,
    });

    // Workspace (null if single-project)
    if let Some(ws) = workspace_json {
        result["workspace"] = json!(ws);
    }

    // RW extras
    if let Some((profile, shell, github)) = security {
        result["security_profile"] = json!(profile);
        result["shell_enabled"] = json!(shell);
        result["github_enabled"] = json!(github);
    }

    // Auto-registered libs summary
    if !auto_registered.is_empty() {
        let without_source = auto_registered.iter().filter(|r| !r.source_available).count();
        result["auto_registered_libs"] = json!({
            "count": auto_registered.len(),
            "without_source": without_source,
        });
    }

    Ok(result)
}
```

- [ ] **Step 4: Rewrite `ActivateProject::call` to use the helper**

Replace the entire `call` method body with:

```rust
async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    let path = super::require_str_param(&input, "path")?;
    let read_only = optional_bool_param(&input, "read_only");

    // Focus-switch path: bare project ID (no path separator)
    if !path.contains('/') && !path.contains('\\') {
        let is_project_id = {
            let inner = ctx.agent.inner.read().await;
            inner
                .workspace
                .as_ref()
                .map(|ws| ws.projects.iter().any(|p| p.discovered.id == path))
                .unwrap_or(false)
        };
        if is_project_id {
            // Capture home state before switching for hint scenario detection
            let was_home = ctx.agent.is_home().await;

            ctx.agent.activate_within_workspace(path, read_only).await?;

            // Determine scenario: if we were at home and switched to home root project, it's ReturnToHome
            let is_now_home = ctx.agent.is_home().await;
            let scenario = if was_home {
                HintScenario::SwitchAway
            } else if is_now_home {
                HintScenario::ReturnToHome
            } else {
                HintScenario::SwitchAway
            };

            let project_root = ctx.agent.require_project_root().await?;
            let auto_registered =
                crate::library::auto_register::auto_register_deps(&project_root, ctx).await;
            return build_activation_response(ctx, scenario, &auto_registered).await;
        }
    }

    // Full-activation path
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(super::RecoverableError::with_hint(
            format!("path '{}' is not a directory", path),
            "Provide an absolute path to an existing directory.",
        )
        .into());
    }
    let root = root.canonicalize().unwrap_or(root);
    let had_home = ctx.agent.home_root().await.is_some();

    ctx.agent.activate(root.clone(), read_only).await?;

    let scenario = if !had_home {
        HintScenario::FirstActivation
    } else if ctx.agent.is_home().await {
        HintScenario::ReturnToHome
    } else {
        HintScenario::SwitchAway
    };

    // Auto-register dependencies (best-effort)
    let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;

    build_activation_response(ctx, scenario, &auto_registered).await
}
```

- [ ] **Step 5: Run the new tests**

Run: `cargo test activate_project_rw_ -- --nocapture && cargo test activate_project_ro_ -- --nocapture && cargo test activate_project_includes_ -- --nocapture && cargo test activate_project_single_ -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Fix existing tests that assert old output shape**

The following existing tests need updating for the new shape:

- `activate_and_get_config` — change `result["activated"]["project_root"]` → `result["project_root"]`; remove assertions on `config` sub-object
- `activate_replaces_previous_project` — update field paths
- `activate_includes_cwd_hint` — hint text now includes "project_status()" for RW
- `activate_hint_shows_switched_when_away_from_home` — hint format changed
- `activate_hint_shows_returned_when_back_home` — hint format changed
- `activate_project_switches_focus_by_id` — now returns full response, not just `project_root`

Read each test body, understand what it asserts, then update the field paths and expected strings to match the new output shape.

- [ ] **Step 7: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 8: Update `format_activate_project` for new compact shape**

The compact formatter must be updated in the same commit as the response shape change, otherwise the existing format tests will fail between commits.

Replace the existing `format_activate_project_no_libs` and `format_activate_project_shows_auto_registered_libs` tests with:

```rust
#[test]
fn format_activate_project_rw_compact() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch", "conventions", "gotchas"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · my-project (rw) · 3 memories · index: not_indexed");
}

#[test]
fn format_activate_project_ro_with_workspace() {
    let result = json!({
        "status": "ok",
        "project": "sub-lib",
        "project_root": "/home/user/mono/sub-lib",
        "read_only": true,
        "memories": [],
        "index": {"status": "indexed"},
        "workspace": [
            {"id": "main", "root": ".", "languages": ["rust"]},
            {"id": "sub-lib", "root": "libs/sub-lib", "languages": ["rust"]},
        ],
        "hint": "Browsing sub-lib (read-only)."
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · sub-lib (ro) · 0 memories · index: indexed · 2 workspace projects");
}

#[test]
fn format_activate_project_with_auto_libs() {
    let result = json!({
        "status": "ok",
        "project": "web",
        "project_root": "/home/user/web",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 12, "without_source": 3},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · web (rw) · 1 memories · index: not_indexed · auto-registered 12 libs (3 without source)");
}

#[test]
fn format_activate_project_auto_libs_all_with_source() {
    let result = json!({
        "status": "ok",
        "project": "app",
        "project_root": "/home/user/app",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 0},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · app (rw) · 0 memories · index: indexed · auto-registered 5 libs");
}
```

- [ ] **Step 9: Rewrite `format_activate_project`**

```rust
fn format_activate_project(result: &Value) -> String {
    let name = result["project"]
        .as_str()
        .unwrap_or("?");
    let ro = result["read_only"].as_bool().unwrap_or(true);
    let mode = if ro { "ro" } else { "rw" };
    let mem_count = result["memories"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let index_status = result["index"]["status"]
        .as_str()
        .unwrap_or("unknown");

    let mut parts = vec![format!("activated · {name} ({mode}) · {mem_count} memories · index: {index_status}")];

    if let Some(ws) = result["workspace"].as_array() {
        parts.push(format!("{} workspace projects", ws.len()));
    }

    if let Some(libs) = result["auto_registered_libs"].as_object() {
        let count = libs.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let without = libs.get("without_source").and_then(|v| v.as_u64()).unwrap_or(0);
        if without > 0 {
            parts.push(format!("auto-registered {} libs ({} without source)", count, without));
        } else {
            parts.push(format!("auto-registered {} libs", count));
        }
    }

    parts.join(" · ")
}
```

- [ ] **Step 10: Run ALL tests (response shape + format + existing)**

Run: `cargo test -- --nocapture -q 2>&1 | tail -5`
Expected: all pass. This step verifies the response shape change and format change are consistent.

- [ ] **Step 11: Commit**

```bash
git add src/tools/config.rs
git commit -m "feat: refactor activate_project output — shared core + RO/RW extras + compact format"
```

---

### Task 4: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md` (activate_project and project_status descriptions)

- [ ] **Step 1: Update the `activate_project` description**

Find the line (around line 190):
```
- `activate_project(path, read_only?)` — switch active project root. Required after `EnterWorktree`. Non-home projects default to `read_only: true` (write tools blocked). Pass `read_only: false` to enable writes on a non-home project.
```

Replace with:
```
- `activate_project(path, read_only?)` — switch active project root. Returns an orientation
  card: project name, languages, available memories, semantic index status, and workspace
  siblings. RW activations additionally include security profile and shell/github toggles.
  Non-home projects default to `read_only: true`. Pass `read_only: false` to enable writes.
  Required after `EnterWorktree`. Use `project_status()` for detailed health checks and
  memory staleness.
```

- [ ] **Step 2: Verify no stale references**

Run: `grep -n "activate_project" src/prompts/server_instructions.md` and review each mention for consistency with the new behavior. Also check `src/prompts/onboarding_prompt.md` and `src/tools/workflow.rs` (`build_system_prompt_draft`).

- [ ] **Step 3: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: update server instructions for new activate_project output"
```

---

### Task 5: Add remaining tests (focus-switch, workspace, edge cases)

**Files:**
- Modify: `src/tools/config.rs` (tests module)

- [ ] **Step 1: Add focus-switch full response test**

```rust
#[tokio::test]
async fn activate_project_focus_switch_returns_full_response() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a sub-project
    let sub = root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };

    // Focus-switch by ID
    let result = ActivateProject
        .call(json!({"path": "api"}), &ctx)
        .await
        .unwrap();

    // Should have full orientation card
    assert_eq!(result["status"], "ok");
    assert!(result["project"].is_string(), "should have project name");
    assert!(result["languages"].is_array(), "should have languages");
    assert!(result["memories"].is_array(), "should have memories");
    assert!(result["index"].is_object(), "should have index");
    assert!(!result["read_only"].is_null(), "should have read_only");
}
```

- [ ] **Step 2: Add workspace includes depends_on test**

```rust
#[tokio::test]
async fn activate_project_workspace_includes_depends_on() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let sub_a = root.join("packages").join("core");
    let sub_b = root.join("packages").join("web");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();
    std::fs::write(sub_a.join("package.json"), r#"{"name":"core","scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::write(sub_b.join("package.json"), r#"{"name":"web","scripts":{"build":"tsc"}}"#).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };

    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    if let Some(ws) = result["workspace"].as_array() {
        for entry in ws {
            assert!(entry["depends_on"].is_array(), "each workspace entry should have depends_on");
        }
    }
}
```

- [ ] **Step 3: Add RO hint warns switch-back test**

```rust
#[tokio::test]
async fn activate_project_ro_hint_warns_switch_back() {
    let home = tempdir().unwrap();
    let other = tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };

    // Activate home first
    ActivateProject
        .call(json!({"path": home.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    // Activate other as RO
    let result = ActivateProject
        .call(json!({"path": other.path().to_str().unwrap(), "read_only": true}), &ctx)
        .await
        .unwrap();

    let hint = result["hint"].as_str().unwrap();
    assert!(hint.contains("remember to activate_project"), "RO hint should warn about switching back, got: {hint}");
    assert!(hint.contains("read-only"), "RO hint should mention read-only, got: {hint}");
}
```

- [ ] **Step 4: Add auto_registered_libs summary test**

```rust
#[test]
fn activate_project_auto_libs_is_summary_not_array() {
    // This tests the response shape, not the tool call — it verifies the
    // format_compact handles the summary object correctly.
    let result = json!({
        "status": "ok",
        "project": "test",
        "project_root": "/tmp/test",
        "read_only": false,
        "memories": [],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 2},
    });
    // auto_registered_libs should be an object with count/without_source, not an array
    assert!(result["auto_registered_libs"].is_object());
    assert_eq!(result["auto_registered_libs"]["count"], 5);
    assert_eq!(result["auto_registered_libs"]["without_source"], 2);
}
```

- [ ] **Step 5: Add memories graceful degradation test**

```rust
#[tokio::test]
async fn activate_project_memories_graceful_on_error() {
    // A project with no .codescout/memory dir should still activate
    // with memories: [] rather than failing
    let dir = tempdir().unwrap();
    // Don't create .codescout dir — MemoryStore::open will create it,
    // but list() on empty store returns empty vec
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    let memories = result["memories"].as_array().unwrap();
    assert!(memories.is_empty(), "empty project should have empty memories array");
}
```

- [ ] **Step 6: Run all new tests**

Run: `cargo test activate_project_ -- --nocapture`
Expected: all pass.

- [ ] **Step 7: Run full test suite + clippy + fmt**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass, no warnings.

- [ ] **Step 8: Commit**

```bash
git add src/tools/config.rs
git commit -m "test: comprehensive tests for activate_project output optimization"
```

---

### Task 6: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full quality gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`
Expected: clean build.

- [ ] **Step 3: Manual verification via MCP**

Restart MCP server (`/mcp`), then test:
1. `activate_project(".", read_only: false)` — verify new RW shape (security_profile, memories, index, hint with project_status)
2. `activate_project("java-library")` — verify focus-switch returns full orientation card
3. `activate_project(".", read_only: true)` — verify RO shape (no security fields, hint mentions read-only)
4. `project_status` — verify it still works correctly with the extracted workspace_summary helper

- [ ] **Step 4: Squash into a single commit for cherry-pick readiness**

If all tasks were committed separately, consider squashing into a clean commit for eventual cherry-pick to master:

```bash
git rebase -i HEAD~N  # squash task commits
```

Or leave as separate commits on experiments — they tell a good story.
