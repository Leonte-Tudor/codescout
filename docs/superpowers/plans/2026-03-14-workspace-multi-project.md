# Workspace Multi-Project Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make codescout workspace-aware so it discovers, reports, and operates on multiple sub-projects within a single repository.

**Architecture:** A new `Workspace` layer above `ActiveProject` with manifest-based project discovery, lazy activation, per-project LSP routing, and project-tagged embedding index. Implemented in 3 phases, each delivering standalone value.

**Tech Stack:** Rust, serde/toml for config, ignore crate for gitignore-respecting walks, rusqlite for schema migration, tokio for async LSP lifecycle.

**Spec:** `docs/superpowers/specs/2026-03-14-workspace-multi-project-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/workspace.rs` | `Workspace`, `DiscoveredProject` types, manifest walk discovery, project resolution helpers |
| `src/config/workspace.rs` | `WorkspaceConfig` serde type for `.codescout/workspace.toml` |

### Modified Files (by phase)

**Phase 1:**
| File | Change |
|------|--------|
| `src/lib.rs` | Add `pub mod workspace;` |
| `src/config/mod.rs` | Add `pub mod workspace;` |
| `src/tools/workflow.rs` | `GatheredContext.projects`, per-project gathering, system prompt with project table |
| `src/tools/config.rs` | `project_status` reports discovered projects |

**Phase 2:**
| File | Change |
|------|--------|
| `src/embed/index.rs` | `project_id` column, migration, tag chunks during indexing |
| `src/tools/semantic.rs` | `project` parameter on `SemanticSearch`, scope extension |
| `src/workspace.rs` | `resolve_project_id(path)` helper for tagging |

**Phase 3+4:**
| File | Change |
|------|--------|
| `src/agent.rs` | `Workspace` replaces `ActiveProject`, `resolve_root()` helper |
| `src/lsp/manager.rs` | `LspKey` re-keying, LRU eviction, idle timeout |
| `src/tools/symbol.rs` | 16 `require_project_root()` sites → `resolve_root()` |
| `src/tools/file.rs` | 3 `require_project_root()` sites → `resolve_root()` |
| `src/tools/config.rs` | `activate_project` dual role, `project_status` workspace view |
| `src/tools/memory.rs` | Per-project memory routing |
| `src/tools/workflow.rs` | `run_command` project-aware cwd |
| `src/tools/mod.rs` | 1 `require_project_root()` site → `resolve_root()` |

---

## Chunk 1: Phase 1 — Workspace Discovery & Multi-Project Onboarding

### Task 1: WorkspaceConfig type

**Files:**
- Create: `src/config/workspace.rs`
- Modify: `src/config/mod.rs`

- [ ] **Step 1: Write the test for WorkspaceConfig deserialization**

In `src/config/workspace.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_workspace_config() {
        let toml_str = r#"
[workspace]
name = "backend-kotlin"
discovery_max_depth = 4

[resources]
max_lsp_clients = 3
idle_timeout_secs = 300

exclude_projects = ["node_modules", "build"]

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["backend-kotlin"]
"#;
        let config: WorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.name, "backend-kotlin");
        assert_eq!(config.workspace.discovery_max_depth, 4);
        assert_eq!(config.resources.max_lsp_clients, 3);
        assert_eq!(config.resources.idle_timeout_secs, 300);
        assert_eq!(config.exclude_projects, vec!["node_modules", "build"]);
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].id, "mcp-server");
        assert_eq!(config.projects[0].depends_on, vec!["backend-kotlin"]);
    }

    #[test]
    fn defaults_are_sensible() {
        let toml_str = r#"
[workspace]
name = "test"
"#;
        let config: WorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.discovery_max_depth, 3);
        assert_eq!(config.resources.max_lsp_clients, 5);
        assert_eq!(config.resources.idle_timeout_secs, 600);
        assert!(config.exclude_projects.is_empty());
        assert!(config.projects.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::workspace::tests -q`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement WorkspaceConfig**

In `src/config/workspace.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub resources: ResourcesSection,
    #[serde(default)]
    pub exclude_projects: Vec<String>,
    #[serde(default, rename = "project")]
    pub projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSection {
    pub name: String,
    #[serde(default = "default_discovery_depth")]
    pub discovery_max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesSection {
    #[serde(default = "default_max_lsp_clients")]
    pub max_lsp_clients: usize,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for ResourcesSection {
    fn default() -> Self {
        Self {
            max_lsp_clients: default_max_lsp_clients(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: String,
    pub root: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_discovery_depth() -> usize { 3 }
fn default_max_lsp_clients() -> usize { 5 }
fn default_idle_timeout() -> u64 { 600 }
```

Add to `src/config/mod.rs`:

```rust
pub mod workspace;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::workspace::tests -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config/workspace.rs src/config/mod.rs
git commit -m "feat(workspace): add WorkspaceConfig type with serde"
```

---

### Task 2: DiscoveredProject type and manifest walk

**Files:**
- Create: `src/workspace.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write tests for manifest discovery**

In `src/workspace.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discover_single_project_repo() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, dir.path().file_name().unwrap().to_str().unwrap());
        assert_eq!(projects[0].relative_root, std::path::Path::new("."));
        assert_eq!(projects[0].manifest, Some("Cargo.toml".to_string()));
    }

    #[test]
    fn discover_multi_project_repo() {
        let dir = tempdir().unwrap();
        // Root: Kotlin
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        // Sub: TypeScript
        let mcp = dir.path().join("mcp-server");
        fs::create_dir_all(&mcp).unwrap();
        fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
        // Sub: Python
        let py = dir.path().join("python-services");
        fs::create_dir_all(&py).unwrap();
        fs::write(py.join("requirements.txt"), "flask\n").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 3);

        // Root project first
        assert_eq!(projects[0].relative_root, std::path::Path::new("."));
        assert_eq!(projects[0].manifest, Some("build.gradle.kts".to_string()));

        // Sub-projects sorted by id
        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"mcp-server"));
        assert!(ids.contains(&"python-services"));
    }

    #[test]
    fn skips_node_modules_manifests() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"scripts":{}}"#).unwrap();
        let nm = dir.path().join("node_modules").join("dep");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.json"), r#"{"name":"dep"}"#).unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 1); // only root, not node_modules/dep
    }

    #[test]
    fn respects_exclude_list() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        fs::write(tools.join("requirements.txt"), "click\n").unwrap();

        let projects = discover_projects(dir.path(), 3, &["tools".to_string()]);
        assert_eq!(projects.len(), 1); // tools excluded
    }

    #[test]
    fn max_depth_limits_discovery() {
        let dir = tempdir().unwrap();
        // Manifest at depth 4 — should be skipped with max_depth=3
        let deep = dir.path().join("a").join("b").join("c").join("deep-service");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Cargo.toml"), "[package]\nname = \"deep\"").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert!(projects.is_empty(), "manifest at depth 4 should be skipped with max_depth=3");

        // But discoverable with max_depth=5
        let projects = discover_projects(dir.path(), 5, &[]);
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn id_collision_is_deduplicated() {
        let dir = tempdir().unwrap();
        // Two subdirectories named "api" at different paths
        let svc_api = dir.path().join("services").join("api");
        fs::create_dir_all(&svc_api).unwrap();
        fs::write(svc_api.join("Cargo.toml"), "[package]\nname = \"svc-api\"").unwrap();

        let tools_api = dir.path().join("tools").join("api");
        fs::create_dir_all(&tools_api).unwrap();
        fs::write(tools_api.join("Cargo.toml"), "[package]\nname = \"tools-api\"").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        // IDs must be unique — one gets the plain name, other gets path-based name
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1], "IDs must be unique: got {:?}", ids);
    }

    #[test]
    fn resolve_project_from_path_uses_longest_prefix() {
        let dir = tempdir().unwrap();
        let projects = vec![
            DiscoveredProject {
                id: "root".into(),
                relative_root: ".".into(),
                languages: vec!["kotlin".into()],
                manifest: Some("build.gradle.kts".into()),
            },
            DiscoveredProject {
                id: "mcp-server".into(),
                relative_root: "mcp-server".into(),
                languages: vec!["typescript".into()],
                manifest: Some("package.json".into()),
            },
        ];

        // File inside mcp-server → resolves to mcp-server
        let result = resolve_project_for_path(
            &projects,
            dir.path(),
            &dir.path().join("mcp-server/src/index.ts"),
        );
        assert_eq!(result.unwrap().id, "mcp-server");

        // File at root → resolves to root
        let result = resolve_project_for_path(
            &projects,
            dir.path(),
            &dir.path().join("src/main/kotlin/App.kt"),
        );
        assert_eq!(result.unwrap().id, "root");
    }

    #[test]
    fn package_json_without_scripts_or_main_is_skipped() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("data");
        fs::create_dir_all(&sub).unwrap();
        // package.json with no scripts/main/module — not a real project
        fs::write(sub.join("package.json"), r#"{"name":"data","version":"1.0"}"#).unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert!(projects.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib workspace::tests -q`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement discovery**

In `src/workspace.rs`:

```rust
use std::path::{Path, PathBuf};

/// A project discovered by manifest walk during onboarding.
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub id: String,
    pub relative_root: PathBuf,
    pub languages: Vec<String>,
    pub manifest: Option<String>,
}

/// Walk the workspace root for build manifests and return discovered sub-projects.
///
/// Rules:
/// - Max depth configurable (default 3)
/// - Respects .gitignore via ignore::WalkBuilder
/// - Root project (if manifest at workspace root) is always first
/// - Skips nested manifests within same-language parent (e.g., node_modules)
/// - Project ID derived from directory name
pub fn discover_projects(
    workspace_root: &Path,
    max_depth: usize,
    exclude: &[String],
) -> Vec<DiscoveredProject> {
    let manifests = [
        ("Cargo.toml", &["rust"][..]),
        ("build.gradle.kts", &["kotlin", "java"]),
        ("build.gradle", &["kotlin", "java"]),
        ("go.mod", &["go"]),
        ("pom.xml", &["java"]),
        ("CMakeLists.txt", &["c", "cpp"]),
        ("mix.exs", &["elixir"]),
        ("Gemfile", &["ruby"]),
    ];
    // These need content validation
    let conditional_manifests = [
        ("package.json", &["typescript", "javascript"][..]),
        ("pyproject.toml", &["python"]),
        ("setup.py", &["python"]),
        ("requirements.txt", &["python"]),
    ];

    let mut found: Vec<DiscoveredProject> = Vec::new();
    let mut found_roots: Vec<PathBuf> = Vec::new(); // track discovered roots to skip nested

    let walker = ignore::WalkBuilder::new(workspace_root)
        .hidden(true)
        .git_ignore(true)
        .max_depth(Some(max_depth))
        .build();

    // Collect all directories that contain a manifest
    let mut manifest_dirs: std::collections::BTreeMap<PathBuf, (String, Vec<String>)> =
        std::collections::BTreeMap::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let dir = entry.path().parent().unwrap_or(entry.path()).to_path_buf();

        // Check if this directory is excluded
        let rel_dir = dir.strip_prefix(workspace_root).unwrap_or(&dir);
        if exclude.iter().any(|ex| {
            rel_dir.components().any(|c| c.as_os_str().to_string_lossy() == *ex)
        }) {
            continue;
        }

        // Check unconditional manifests
        for (manifest_name, langs) in &manifests {
            if file_name == *manifest_name && !manifest_dirs.contains_key(&dir) {
                manifest_dirs.insert(
                    dir.clone(),
                    (manifest_name.to_string(), langs.iter().map(|s| s.to_string()).collect()),
                );
            }
        }

        // Check conditional manifests
        for (manifest_name, langs) in &conditional_manifests {
            if file_name != *manifest_name || manifest_dirs.contains_key(&dir) {
                continue;
            }
            if *manifest_name == "package.json" {
                // Only count if has scripts, main, or module
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let has_scripts = json.get("scripts").and_then(|v| v.as_object()).map(|o| !o.is_empty()).unwrap_or(false);
                        let has_main = json.get("main").is_some() || json.get("module").is_some();
                        if !has_scripts && !has_main {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if *manifest_name == "requirements.txt" {
                // Skip if pyproject.toml sibling exists
                if dir.join("pyproject.toml").exists() {
                    continue;
                }
            }
            manifest_dirs.insert(
                dir.clone(),
                (manifest_name.to_string(), langs.iter().map(|s| s.to_string()).collect()),
            );
        }
    }

    // Convert to DiscoveredProject, filtering nested same-language projects
    let workspace_name = workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    // Sort by path depth (shallowest first) so parents are processed before children
    let mut dirs: Vec<_> = manifest_dirs.into_iter().collect();
    dirs.sort_by_key(|(p, _)| p.components().count());

    for (dir, (manifest, languages)) in dirs {
        let rel = dir.strip_prefix(workspace_root).unwrap_or(&dir);
        let rel_path = if rel.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            rel.to_path_buf()
        };

        // Skip if nested inside an already-found project with overlapping languages
        let dominated = found_roots.iter().any(|existing| {
            if rel_path == PathBuf::from(".") || existing == &PathBuf::from(".") {
                return false; // root never dominates, and is never dominated
            }
            rel_path.starts_with(existing)
                && found.iter().any(|p| {
                    p.relative_root == *existing
                        && p.languages.iter().any(|l| languages.contains(l))
                })
        });
        if dominated {
            continue;
        }

        let id = if rel_path == PathBuf::from(".") {
            workspace_name.clone()
        } else {
            rel_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string()
        };

        // Deduplicate IDs by appending parent if needed
        let final_id = if found.iter().any(|p| p.id == id) {
            rel_path.to_string_lossy().replace('/', "-")
        } else {
            id
        };

        found_roots.push(rel_path.clone());
        found.push(DiscoveredProject {
            id: final_id,
            relative_root: rel_path,
            languages,
            manifest: Some(manifest),
        });
    }

    // Ensure root project is first
    if let Some(root_idx) = found.iter().position(|p| p.relative_root == PathBuf::from(".")) {
        if root_idx != 0 {
            let root = found.remove(root_idx);
            found.insert(0, root);
        }
    }

    found
}

/// Resolve which project a file path belongs to using longest-prefix match.
pub fn resolve_project_for_path<'a>(
    projects: &'a [DiscoveredProject],
    workspace_root: &Path,
    file_path: &Path,
) -> Option<&'a DiscoveredProject> {
    let abs_file = if file_path.is_relative() {
        workspace_root.join(file_path)
    } else {
        file_path.to_path_buf()
    };

    projects
        .iter()
        .filter(|p| {
            let project_abs = if p.relative_root == PathBuf::from(".") {
                workspace_root.to_path_buf()
            } else {
                workspace_root.join(&p.relative_root)
            };
            abs_file.starts_with(&project_abs)
        })
        .max_by_key(|p| p.relative_root.components().count())
}
```

Add to `src/lib.rs`:

```rust
pub mod workspace;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib workspace::tests -q`
Expected: PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/workspace.rs src/lib.rs
git commit -m "feat(workspace): manifest-based project discovery with longest-prefix resolution"
```

---

### Task 3: Integrate discovery into onboarding

**Files:**
- Modify: `src/tools/workflow.rs` (GatheredContext, gather_project_context, Onboarding::call, build_system_prompt_draft)

- [ ] **Step 1: Write test for multi-project onboarding output**

Add to `src/tools/workflow.rs` tests module:

```rust
#[tokio::test]
async fn onboarding_discovers_sub_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Root: Kotlin
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    std::fs::create_dir_all(root.join("src/main/kotlin")).unwrap();
    std::fs::write(root.join("src/main/kotlin/App.kt"), "fun main() {}").unwrap();

    // Sub: TypeScript
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(mcp.join("src")).unwrap();
    std::fs::write(
        mcp.join("package.json"),
        r#"{"scripts":{"build":"tsc"}}"#,
    ).unwrap();
    std::fs::write(mcp.join("src/index.ts"), "").unwrap();

    // Sub: Python
    let py = root.join("python-services");
    std::fs::create_dir_all(&py).unwrap();
    std::fs::write(py.join("requirements.txt"), "flask\n").unwrap();
    std::fs::write(py.join("app.py"), "").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = Arc::new(crate::lsp::mock::MockLspProvider::new());
    let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::default());
    let progress = crate::tools::progress::ProgressReporter::new(None);
    let ctx = ToolContext { agent: agent.clone(), lsp, output_buffer, progress };

    let result = Onboarding.call(json!({"force": true}), &ctx).await.unwrap();

    // Should have discovered projects
    let projects = result.get("projects").expect("onboarding should return projects");
    let projects_arr = projects.as_array().unwrap();
    assert_eq!(projects_arr.len(), 3, "should discover 3 projects (root + mcp-server + python-services), got {}", projects_arr.len());

    // System prompt draft should mention projects
    let draft = result["system_prompt_draft"].as_str().unwrap();
    assert!(draft.contains("mcp-server"), "draft should mention mcp-server");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib onboarding_discovers_sub_projects -q`
Expected: FAIL — `projects` field not in output

- [ ] **Step 3: Add `projects` field to GatheredContext**

In `src/tools/workflow.rs`, modify the `GatheredContext` struct:

```rust
struct GatheredContext {
    readme_path: Option<String>,
    build_file_name: Option<String>,
    claude_md_exists: bool,
    ci_files: Vec<String>,
    entry_points: Vec<String>,
    test_dirs: Vec<String>,
    features_md: Option<String>,
    projects: Vec<crate::workspace::DiscoveredProject>,  // NEW
}
```

- [ ] **Step 4: Call discovery in `gather_project_context`**

At the end of `gather_project_context()`, before `ctx` is returned, add:

```rust
// Discover sub-projects via manifest walk
ctx.projects = crate::workspace::discover_projects(root, 3, &Vec::new());
```

- [ ] **Step 5: Add projects to onboarding output and system prompt draft**

In `Onboarding::call`, after `let gathered = gather_project_context(&root);`, add the projects to the JSON output:

```rust
let discovered_projects: Vec<serde_json::Value> = gathered.projects.iter().map(|p| {
    json!({
        "id": p.id,
        "root": p.relative_root.to_string_lossy(),
        "languages": p.languages,
        "manifest": p.manifest,
    })
}).collect();
```

Add `"projects": discovered_projects` to the final `json!({...})` return.

In `build_system_prompt_draft`, add an optional projects parameter with `None` default for
backward compatibility. **Important**: find all existing call sites of `build_system_prompt_draft`
(use `find_references`) and add `, None` to each. This avoids breaking existing tests.

```rust
// Add workspace projects table if multi-project
fn build_system_prompt_draft(
    languages: &[String],
    entry_points: &[String],
    project_root: Option<&Path>,
    projects: Option<&[crate::workspace::DiscoveredProject]>,  // NEW param — None = no workspace
) -> String {
    // ... existing code ...

    // After the Language Navigation section, before Navigation Strategy:
    let projects = projects.unwrap_or(&[]);
    if projects.len() > 1 {
        draft.push_str("## Workspace Projects\n\n");
        draft.push_str("| Project | Root | Languages | Build |\n");
        draft.push_str("|---------|------|-----------|-------|\n");
        for p in projects {
            draft.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                p.id,
                p.relative_root.display(),
                p.languages.join(", "),
                p.manifest.as_deref().unwrap_or("-"),
            ));
        }
        draft.push('\n');

        // Render depends_on from workspace.toml if available
        // (depends_on is user-edited in workspace.toml, not auto-discovered)
        // Load workspace.toml and check for depends_on entries
        if let Some(root) = project_root {
            let ws_path = root.join(".codescout").join("workspace.toml");
            if let Ok(content) = std::fs::read_to_string(&ws_path) {
                if let Ok(ws) = toml::from_str::<crate::config::workspace::WorkspaceConfig>(&content) {
                    let deps: Vec<_> = ws.projects.iter()
                        .filter(|p| !p.depends_on.is_empty())
                        .collect();
                    if !deps.is_empty() {
                        draft.push_str("**Cross-project dependencies:**\n");
                        for p in deps {
                            draft.push_str(&format!(
                                "- {} depends on {}\n",
                                p.id,
                                p.depends_on.join(", "),
                            ));
                        }
                        draft.push('\n');
                    }
                }
            }
        }

        draft.push_str(
            "Use `project: \"name\"` parameter to scope search/navigation to a specific project.\n\n"
        );
    }

    // ... rest of existing code ...
}
```

Update the call site in `Onboarding::call` to pass `Some(&gathered.projects)`.
Update all other existing call sites to pass `None` for the new parameter (use `find_references`
on `build_system_prompt_draft` to find them all).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --lib onboarding_discovers_sub_projects -q`
Expected: PASS

- [ ] **Step 7: Fix any broken existing tests**

Run: `cargo test --lib -q`
Expected: PASS (existing tests may need `projects: vec![]` added to `build_system_prompt_draft` calls)

- [ ] **Step 8: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(workspace): multi-project discovery in onboarding output and system prompt"
```

---

### Task 4: Write workspace.toml on multi-project onboarding

**Files:**
- Modify: `src/tools/workflow.rs` (Onboarding::call)

- [ ] **Step 1: Write test for workspace.toml creation**

```rust
#[tokio::test]
async fn onboarding_creates_workspace_toml_for_multi_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Root: Kotlin
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/App.kt"), "").unwrap();

    // Sub: TypeScript
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = Arc::new(crate::lsp::mock::MockLspProvider::new());
    let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::default());
    let progress = crate::tools::progress::ProgressReporter::new(None);
    let ctx = ToolContext { agent, lsp, output_buffer, progress };

    Onboarding.call(json!({"force": true}), &ctx).await.unwrap();

    // workspace.toml should exist
    let ws_path = root.join(".codescout").join("workspace.toml");
    assert!(ws_path.exists(), "workspace.toml should be created for multi-project repos");

    let content = std::fs::read_to_string(&ws_path).unwrap();
    let config: crate::config::workspace::WorkspaceConfig = toml::from_str(&content).unwrap();
    assert_eq!(config.projects.len(), 2); // root + mcp-server
}

#[tokio::test]
async fn onboarding_skips_workspace_toml_for_single_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = Arc::new(crate::lsp::mock::MockLspProvider::new());
    let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::default());
    let progress = crate::tools::progress::ProgressReporter::new(None);
    let ctx = ToolContext { agent, lsp, output_buffer, progress };

    Onboarding.call(json!({"force": true}), &ctx).await.unwrap();

    let ws_path = root.join(".codescout").join("workspace.toml");
    assert!(!ws_path.exists(), "workspace.toml should NOT be created for single-project repos");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib workspace_toml -q`
Expected: FAIL

- [ ] **Step 3: Implement workspace.toml creation in Onboarding::call**

In `Onboarding::call`, after `let gathered = gather_project_context(&root);`, add:

```rust
// Create workspace.toml for multi-project repos
let workspace_config_path = config_dir.join("workspace.toml");
if gathered.projects.len() > 1 && !workspace_config_path.exists() {
    let ws_config = crate::config::workspace::WorkspaceConfig {
        workspace: crate::config::workspace::WorkspaceSection {
            name: root.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string(),
            discovery_max_depth: 3,
        },
        resources: Default::default(),
        exclude_projects: vec![],
        projects: gathered.projects.iter().map(|p| {
            crate::config::workspace::ProjectEntry {
                id: p.id.clone(),
                root: p.relative_root.to_string_lossy().to_string(),
                languages: p.languages.clone(),
                depends_on: vec![],
            }
        }).collect(),
    };
    let toml_str = toml::to_string_pretty(&ws_config)?;
    std::fs::write(&workspace_config_path, &toml_str)?;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib workspace_toml -q`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test -q`
Expected: PASS

- [ ] **Step 6: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(workspace): create workspace.toml for multi-project repos on onboarding"
```

---

### Task 5: project_status reports discovered projects

**Files:**
- Modify: `src/tools/config.rs` (ProjectStatus::call)

- [ ] **Step 1: Write test**

```rust
#[tokio::test]
async fn project_status_shows_workspace_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create multi-project structure
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    // Create workspace.toml
    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["test"]
"#,
    ).unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    ).unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = Arc::new(crate::lsp::mock::MockLspProvider::new());
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::default()),
        progress: crate::tools::progress::ProgressReporter::new(None),
    };

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let ws = result.get("workspace");
    assert!(ws.is_some(), "project_status should include workspace section");
    let projects = ws.unwrap().get("projects").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib project_status_shows_workspace -q`
Expected: FAIL

- [ ] **Step 3: Add workspace info to ProjectStatus::call**

In `ProjectStatus::call`, after loading the project, check for `workspace.toml` and include its data:

```rust
// Load workspace config if present
let workspace_toml_path = root.join(".codescout").join("workspace.toml");
let workspace_info = if workspace_toml_path.exists() {
    match std::fs::read_to_string(&workspace_toml_path)
        .ok()
        .and_then(|s| toml::from_str::<crate::config::workspace::WorkspaceConfig>(&s).ok())
    {
        Some(ws) => Some(json!({
            "name": ws.workspace.name,
            "projects": ws.projects.iter().map(|p| json!({
                "id": p.id,
                "root": p.root,
                "languages": p.languages,
                "depends_on": p.depends_on,
            })).collect::<Vec<_>>(),
            "resources": {
                "max_lsp_clients": ws.resources.max_lsp_clients,
                "idle_timeout_secs": ws.resources.idle_timeout_secs,
            },
        })),
        None => None,
    }
} else {
    None
};
```

Add `"workspace": workspace_info` to the return JSON.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib project_status_shows_workspace -q && cargo test --lib -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/config.rs
git commit -m "feat(workspace): project_status reports workspace projects"
```

---

### Task 6: Phase 1 integration test and final verification

**Files:**
- No new files — verification only

- [ ] **Step 1: Run full test suite**

Run: `cargo test -q`
Expected: ALL PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: PASS

- [ ] **Step 4: Manual verification with backend-kotlin**

Start the release binary against backend-kotlin and run onboarding:
```bash
cargo run --release -- start --project /home/marius/work/mirela/backend-kotlin
```

Verify:
1. `onboarding(force: true)` discovers 3 projects (backend-kotlin, mcp-server, python-services)
2. System prompt draft includes a "Workspace Projects" table
3. `workspace.toml` is created at `.codescout/workspace.toml`
4. `project_status` shows workspace section with all 3 projects

- [ ] **Step 5: Commit any fixes from manual testing**

---

## Chunk 2: Phase 2 — Project-Tagged Embedding Index

### Task 7: Add project_id column to chunks table

**Files:**
- Modify: `src/embed/index.rs` (open_db, schema migration)

- [ ] **Step 1: Write test for migration**

```rust
#[test]
fn open_db_adds_project_id_column() {
    let dir = tempdir().unwrap();
    // First open creates the DB without project_id
    let conn = open_db(dir.path()).unwrap();
    // Verify column exists
    let has_col: bool = conn
        .prepare("SELECT project_id FROM chunks LIMIT 0")
        .is_ok();
    assert!(has_col, "chunks table should have project_id column");
}

#[test]
fn existing_chunks_get_default_root_project_id() {
    let dir = tempdir().unwrap();
    let conn = open_db(dir.path()).unwrap();

    // Insert a chunk the old way (without project_id)
    conn.execute(
        "INSERT INTO chunks (file_path, start_line, end_line, content, language, file_hash, source)
         VALUES ('src/main.rs', 1, 10, 'fn main() {}', 'rust', 'abc123', 'project')",
        [],
    ).unwrap();

    // Check default
    let pid: String = conn
        .query_row("SELECT project_id FROM chunks WHERE file_path = 'src/main.rs'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pid, "root");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib open_db_adds_project_id -q`
Expected: FAIL — column doesn't exist yet

- [ ] **Step 3: Add migration in open_db**

In `src/embed/index.rs`, inside `open_db()`, after the existing `has_source` migration block, add:

```rust
// Migration: add project_id column (workspace multi-project support)
let has_project_id: bool = conn
    .prepare("SELECT project_id FROM chunks LIMIT 0")
    .is_ok();
if !has_project_id {
    conn.execute(
        "ALTER TABLE chunks ADD COLUMN project_id TEXT NOT NULL DEFAULT 'root'",
        [],
    )?;
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib open_db_adds_project_id -q && cargo test --lib existing_chunks_get_default -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(workspace): add project_id column to chunks table with migration"
```

---

### Task 8: Tag chunks with project_id during indexing

**Files:**
- Modify: `src/embed/index.rs` (insert_chunk, build_index)
- Modify: `src/workspace.rs` (add resolve_project_id helper)

- [ ] **Step 1: Add resolve_project_id helper to workspace.rs**

```rust
/// Given a file path and a list of discovered projects, return the project ID.
/// Falls back to "root" if no project claims the file.
pub fn resolve_project_id(
    projects: &[DiscoveredProject],
    workspace_root: &Path,
    file_path: &Path,
) -> String {
    resolve_project_for_path(projects, workspace_root, file_path)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| "root".to_string())
}
```

- [ ] **Step 2: Modify insert_chunk to accept project_id**

In `src/embed/index.rs`, find the `insert_chunk` function (or equivalent INSERT statement in `build_index`) and add `project_id` to the INSERT:

```sql
INSERT INTO chunks (file_path, start_line, end_line, content, language, file_hash, source, project_id)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
```

The caller (build_index) resolves the project_id from the workspace discovery data and passes it through.

- [ ] **Step 3: Write test for tagged indexing**

```rust
#[test]
fn build_index_tags_chunks_with_project_id() {
    // This test requires a workspace structure with multiple projects
    // and verifies chunks are tagged correctly after indexing.
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create two sub-projects
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"test\"").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let sub = root.join("frontend");
    std::fs::create_dir_all(sub.join("src")).unwrap();
    std::fs::write(sub.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::write(sub.join("src/index.ts"), "export const x = 1;").unwrap();

    // Discover projects
    let projects = crate::workspace::discover_projects(root, 3, &[]);
    assert!(projects.len() >= 2);

    // After build_index with project awareness, verify tags
    let conn = open_db(root).unwrap();
    // ... (insert test chunks manually with project_id resolved from workspace)
    // Verify: src/main.rs → root project, frontend/src/index.ts → "frontend"
}
```

- [ ] **Step 4: Run tests and fix**

Run: `cargo test --lib build_index_tags -q`

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs src/workspace.rs
git commit -m "feat(workspace): tag chunks with project_id during indexing"
```

---

### Task 9: Add project parameter to SemanticSearch

**Files:**
- Modify: `src/tools/semantic.rs` (SemanticSearch::input_schema, SemanticSearch::call)

- [ ] **Step 1: Write test for project-scoped search**

```rust
#[test]
fn semantic_search_schema_has_project() {
    let schema = SemanticSearch.input_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("project"), "schema should have project param");
}
```

- [ ] **Step 2: Verify failure, implement, verify pass**

Add `"project"` to `input_schema()`:
```json
"project": {
    "type": "string",
    "description": "Filter results to a specific project (e.g. 'mcp-server'). Default: all projects."
}
```

In `SemanticSearch::call`, extract the project param and pass it to the search query as a `WHERE project_id = ?` filter when present.

Extend the `scope` parameter parsing to handle `"project"` (focused project) and `"project:<name>"` (specific project).

- [ ] **Step 3: Run full test suite**

Run: `cargo test -q`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(workspace): add project parameter to semantic_search for scoped queries"
```

---

### Task 10: Phase 2 verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -q && cargo clippy -- -D warnings`

- [ ] **Step 2: Build release**

Run: `cargo build --release`

- [ ] **Step 3: Manual verification**

Index backend-kotlin and verify:
1. `index_project` tags chunks with correct project IDs
2. `semantic_search(query: "authentication", project: "mcp-server")` returns only TS results
3. `semantic_search(query: "authentication")` returns results from all projects

---

## Chunk 3: Phase 3+4 — Agent Model + LSP Re-Keying

> **Note**: This is the riskiest chunk. Phases 3 and 4 MUST ship together (see spec: LSP overwrite bug). This chunk is outlined at a higher level than Chunks 1-2 because the exact code depends on decisions made during Phase 1-2 implementation.

### Task 11: Workspace struct in agent.rs

**Files:**
- Modify: `src/agent.rs`
- Modify: `src/workspace.rs` (add `ProjectState` enum)

- [ ] **Step 1: Define types**

Add `ProjectState` to `src/workspace.rs`:

```rust
pub enum ProjectState {
    Dormant,
    Activated {
        config: crate::config::project::ProjectConfig,
        memory: crate::memory::MemoryStore,
        private_memory: crate::memory::MemoryStore,
        library_registry: crate::library::LibraryRegistry,
        dirty_files: Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>,
    },
}

pub struct Project {
    pub discovered: DiscoveredProject,
    pub state: ProjectState,
}

pub struct Workspace {
    pub root: PathBuf,
    pub config: crate::config::workspace::WorkspaceConfig,  // for resource caps
    pub projects: Vec<Project>,
    pub focused: Option<String>,  // project ID
}
```

- [ ] **Step 2: Write tests for `Workspace::resolve_root`**

```rust
impl Workspace {
    pub fn resolve_root(&self, project: Option<&str>, file_hint: Option<&Path>) -> Result<PathBuf> {
        match (project, file_hint) {
            (Some(id), _) => self.project_root_by_id(id),
            (None, Some(path)) => Ok(self.resolve_project_root_from_path(path)),
            (None, None) => self.focused_project_root(),
        }
    }
}
```

Test: explicit project ID → correct root. File hint → longest prefix match. None → focused.

- [ ] **Step 3: Replace `AgentInner.active_project` with `workspace`**

Change `AgentInner`:
```rust
pub struct AgentInner {
    pub workspace: Option<Workspace>,
    pub project_explicitly_activated: bool,
    pub home_root: Option<PathBuf>,
}
```

- [ ] **Step 4: Update `require_project_root` to delegate to workspace**

```rust
pub async fn require_project_root(&self) -> Result<PathBuf> {
    let inner = self.inner.read().await;
    inner.workspace.as_ref()
        .and_then(|ws| ws.focused_project_root().ok())
        .ok_or_else(|| RecoverableError::with_hint(
            "No active project.",
            "Call activate_project to set the active project.",
        ).into())
}
```

- [ ] **Step 5: Add `resolve_root` to Agent**

```rust
pub async fn resolve_root(&self, project: Option<&str>, file_hint: Option<&Path>) -> Result<PathBuf> {
    let inner = self.inner.read().await;
    inner.workspace.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No active project"))?
        .resolve_root(project, file_hint)
}
```

- [ ] **Step 6: Run tests, fix all 24+ call sites incrementally**

Each call site in `src/tools/*.rs` that uses `require_project_root()` continues to work unchanged (backward compatible). The `resolve_root` method is available for tools that want project-aware routing.

**Migration note**: Phase 1 code accesses `DiscoveredProject` fields directly (e.g., `p.id`,
`p.languages`). In Phase 3, `Project` wraps `DiscoveredProject` as `p.discovered`. All Phase 1
code in onboarding output and project_status that iterates over projects will need updating to
go through `p.discovered.*`. Search for all `DiscoveredProject` field accesses before this step.

**Design note**: `Workspace::resolve_root` is intentionally synchronous (pure lookup on
in-memory data), unlike the spec's `async fn` signature on Agent. The async wrapper on
`Agent::resolve_root` is just for locking `inner.read().await`.

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(workspace): replace ActiveProject with Workspace in agent model"
```

---

### Task 12: LSP Manager re-keying

**Files:**
- Modify: `src/lsp/manager.rs`

- [ ] **Step 1: Define LspKey**

```rust
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct LspKey {
    pub language: String,
    pub project_root: PathBuf,
}
```

- [ ] **Step 2: Change clients HashMap key**

```rust
clients: Mutex<HashMap<LspKey, Arc<LspClient>>>
```

- [ ] **Step 3: Update `get_or_start` to use LspKey**

The signature stays the same (`language: &str, workspace_root: &Path`), but internally it constructs `LspKey { language, project_root: workspace_root }` for cache lookup.

- [ ] **Step 4: Add LRU tracking**

Add `last_used: Mutex<HashMap<LspKey, std::time::Instant>>` to LspManager. Update on every `get_or_start` hit. When `clients.len() >= max_lsp_clients`, evict the oldest before starting a new one.

- [ ] **Step 5: Add idle timeout**

A background `tokio::spawn` task that periodically (every 60s) checks `last_used` and shuts down clients idle for longer than `idle_timeout_secs`.

- [ ] **Step 6: Update barrier/dedup to key on LspKey**

The `starting: Mutex<HashMap<String, ...>>` becomes `HashMap<LspKey, ...>`.

- [ ] **Step 7: Scope notify_file_changed**

When notifying file changes, only notify LSP clients whose `project_root` contains the changed file.

- [ ] **Step 8: Write tests**

- Two different project roots for the same language get separate clients
- LRU eviction works when cap is hit
- Idle timeout shuts down unused clients
- Barrier dedup prevents double-start for same LspKey

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(workspace): LSP manager re-keyed by (language, project_root) with LRU eviction"
```

---

### Task 13: activate_project dual role

**Files:**
- Modify: `src/tools/config.rs`

- [ ] **Step 1: Implement disambiguation**

```rust
// In ActivateProject::call
let arg = input["path"].as_str().unwrap_or("");

// Check if it's a project ID (focus switch within current workspace)
let is_focus_switch = ctx.agent.inner.read().await
    .workspace.as_ref()
    .map(|ws| ws.projects.iter().any(|p| p.discovered.id == arg))
    .unwrap_or(false);

if is_focus_switch {
    // Switch focus to named project, lazy-activate if dormant
    // ...
} else {
    // Treat as path — init new workspace or switch workspaces
    // ...
}
```

- [ ] **Step 2: Write tests**

- `activate_project("mcp-server")` on an existing workspace switches focus
- `activate_project("/new/path")` initializes a new workspace
- Unknown ID with no `/` returns a helpful error

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(workspace): activate_project dual role — path or project ID"
```

---

### Task 14: Per-project memory directories

**Files:**
- Modify: `src/tools/memory.rs`
- Modify: `src/workspace.rs` (memory path helpers)

- [ ] **Step 1: Add memory path resolution**

```rust
// In Workspace
pub fn memory_dir_for_project(&self, project_id: &str) -> PathBuf {
    if self.projects.iter().find(|p| p.discovered.id == project_id)
        .map(|p| p.discovered.relative_root == PathBuf::from("."))
        .unwrap_or(true)
    {
        // Root project — use workspace-level memories
        self.root.join(".codescout").join("memories")
    } else {
        // Sub-project — use per-project memories
        self.root.join(".codescout").join("projects").join(project_id).join("memories")
    }
}
```

- [ ] **Step 2: Update memory tool to route based on project param**

When `project` parameter is provided, use the project-specific memory directory. When `recall` is called without project, search across all project memories.

- [ ] **Step 3: Write tests and commit**

```bash
git commit -m "feat(workspace): per-project memory directories"
```

---

### Task 15: Phase 3+4 integration test and verification

- [ ] **Step 1: Full test suite**

Run: `cargo test -q && cargo clippy -- -D warnings`

- [ ] **Step 2: Release build**

Run: `cargo build --release`

- [ ] **Step 3: Manual verification with backend-kotlin**

Via MCP (`/mcp` restart after release build):

1. `activate_project("/home/marius/work/mirela/backend-kotlin")` → workspace with 3 projects
2. `find_symbol("AuthService")` → searches in focused project (Kotlin)
3. `activate_project("mcp-server")` → switches focus to TypeScript
4. `find_symbol("handleRequest")` → uses TypeScript LSP rooted at `mcp-server/`
5. `semantic_search("authentication", project: "python-services")` → Python results only
6. `memory(action: "write", topic: "conventions", project: "mcp-server")` → writes to per-project dir
7. `project_status` → shows all 3 projects, their states, LSP status

- [ ] **Step 4: Commit any fixes**

---

## Summary

| Chunk | Tasks | Estimated Scope | Risk |
|-------|-------|----------------|------|
| 1 (Phase 1) | Tasks 1-6 | ~500 lines | Low |
| 2 (Phase 2) | Tasks 7-10 | ~300 lines | Medium |
| 3 (Phase 3+4) | Tasks 11-15 | ~1200 lines | High |

Phase 1 alone solves the original pain point (onboarding awareness). Ship it first, validate with `backend-kotlin`, then proceed to Phase 2 and 3+4.
