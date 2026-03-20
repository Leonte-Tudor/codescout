//! Configuration and project management tools.

use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ActivateProject;
pub struct ProjectStatus;

#[async_trait::async_trait]
impl Tool for ActivateProject {
    fn name(&self) -> &str {
        "activate_project"
    }
    fn description(&self) -> &str {
        "Switch the active project to the given path. All subsequent tool calls \
         operate relative to this project root."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the project root" },
                "read_only": { "type": "boolean", "description": "Activate in read-only mode (default: true for non-home projects, false for home)" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let path = super::require_str_param(&input, "path")?;

        // If the argument contains no path separator and matches a known project ID
        // in the current workspace, switch focus without reinitialising.
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
                // Capture home state before switching so we can warn about returning
                let home_root = ctx.agent.home_root().await;
                let was_home = ctx.agent.is_home().await;

                ctx.agent.switch_focus(path).await?;
                let project_root = ctx.agent.require_project_root().await?;

                let hint = if was_home {
                    if let Some(ref home) = home_root {
                        format!(
                            "Switched focus to '{}'. CWD: {} — ⚠ remember to \
                             activate_project(\"{}\") when done (server state is \
                             shared with parent conversation)",
                            path,
                            project_root.display(),
                            home.display(),
                        )
                    } else {
                        format!(
                            "Switched focus to '{}'. CWD: {}",
                            path,
                            project_root.display()
                        )
                    }
                } else {
                    format!(
                        "Switched focus to '{}'. CWD: {}",
                        path,
                        project_root.display()
                    )
                };

                return Ok(json!({
                    "status": "ok",
                    "activated": {
                        "project_root": project_root.display().to_string(),
                    },
                    "hint": hint,
                }));
            }
        }

        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("path '{}' is not a directory", path),
                "Provide an absolute path to an existing directory.",
            )
            .into());
        }
        // Canonicalize so relative paths like "." resolve to absolute paths,
        // enabling correct home_root comparison in agent.activate().
        let root = root.canonicalize().unwrap_or(root);
        // Capture before activate() — activate sets home_root on first call
        let had_home = ctx.agent.home_root().await.is_some();

        let read_only = optional_bool_param(&input, "read_only");
        ctx.agent.activate(root.clone(), read_only).await?;

        let config = ctx
            .agent
            .with_project(|p| {
                Ok(json!({
                    "project_root": p.root.display().to_string(),
                    "config": serde_json::to_value(&p.config)?,
                }))
            })
            .await?;

        // Build CWD hint
        let project_root_str = config["project_root"].as_str().unwrap_or("?");
        let is_home = ctx.agent.is_home().await;
        let home = ctx.agent.home_root().await;
        let is_read_only = ctx
            .agent
            .with_project(|p| Ok(p.read_only))
            .await
            .unwrap_or(false);

        let hint = if !had_home {
            format!("CWD: {}", project_root_str)
        } else if is_home {
            format!("Returned to original project. CWD: {}", project_root_str)
        } else {
            let home_str = home
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let ro_notice = if is_read_only {
                " This project is activated in read-only mode. To enable writes, \
                 call activate_project with read_only: false."
            } else {
                ""
            };
            format!(
                "Switched project. CWD: {} — ⚠ remember to activate_project(\"{}\") \
                 when done (server state is shared with parent conversation).{}",
                project_root_str, home_str, ro_notice,
            )
        };

        // Auto-register Cargo dependencies (best-effort, never fails activation)
        let auto_registered = auto_register_cargo_deps(&root, ctx).await;

        let mut result =
            json!({ "status": "ok", "activated": config, "read_only": is_read_only, "hint": hint });
        if !auto_registered.is_empty() {
            result["auto_registered_libs"] = json!(auto_registered);
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_activate_project(result))
    }
}

#[async_trait::async_trait]
impl Tool for ProjectStatus {
    fn name(&self) -> &str {
        "project_status"
    }

    fn description(&self) -> &str {
        "Active project state: languages, embedding model, index health summary, and memory staleness. \
         Pass post_compact=true after context compaction to flush stale LSP position caches — \
         clients restart lazily on the next LSP call. \
         Call index_status() for detailed index info and live progress."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "post_compact": {
                    "type": "boolean",
                    "description": "Set true after context compaction to flush stale LSP position caches. \
                                    LSP clients restart lazily on the next navigation call."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::agent::IndexingState;

        // --- PostCompact cache flush ---
        if parse_bool_param(&input["post_compact"]) {
            ctx.lsp.shutdown_all().await;
            tracing::info!("PostCompact: flushed all LSP clients; they will restart lazily.");
            return Ok(json!({
                "flushed": true,
                "hint": "LSP position caches cleared. Clients restart automatically on the next navigation call (goto_definition, hover, find_references)."
            }));
        }

        // --- Essential config + library section ---
        let (root, languages, embeddings_model, lib_count, lib_indexed) = ctx
            .agent
            .with_project(|p| {
                let lib_count = p.library_registry.all().len();
                let lib_indexed = p
                    .library_registry
                    .all()
                    .iter()
                    .filter(|e| e.indexed)
                    .count();
                Ok((
                    p.root.clone(),
                    p.config.project.languages.clone(),
                    p.config.embeddings.model.clone(),
                    lib_count,
                    lib_indexed,
                ))
            })
            .await?;

        let mut result = json!({
            "project_root": root.display().to_string(),
            "languages": languages,
            "embeddings_model": embeddings_model,
            "libraries": { "count": lib_count, "indexed": lib_indexed },
        });

        // --- Index section ---
        // Running state takes priority over DB stats.
        let indexing_state = ctx
            .agent
            .indexing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if let IndexingState::Running {
            done,
            total,
            eta_secs,
        } = indexing_state
        {
            result["index"] = json!({
                "status": "running",
                "done": done,
                "total": total,
                "eta_secs": eta_secs,
                "hint": "Call index_status() for detailed breakdown.",
            });
        } else {
            let db_path = crate::embed::index::project_db_path(&root);
            if !db_path.exists() {
                result["index"] = json!({
                    "status": "not_indexed",
                    "hint": "Run index_project() to build the index.",
                });
            } else {
                let root2 = root.clone();
                let index_result = tokio::task::spawn_blocking(move || {
                    let conn = crate::embed::index::open_db(&root2)?;
                    let stats = crate::embed::index::index_stats(&conn)?;
                    let staleness = crate::embed::index::check_index_staleness(&conn, &root2).ok();
                    anyhow::Ok((stats, staleness))
                })
                .await;

                match index_result {
                    Ok(Ok((stats, staleness))) => {
                        let status = match staleness.as_ref() {
                            Some(s) if s.stale => "behind",
                            _ => "up_to_date",
                        };
                        result["index"] = json!({
                            "status": status,
                            "files": stats.file_count,
                            "chunks": stats.chunk_count,
                            "last_updated": stats.indexed_at,
                            "hint": "Call index_status() for model info, by_source breakdown, drift, and progress details.",
                        });
                    }
                    _ => {
                        result["index"] = json!({
                            "status": "not_indexed",
                            "hint": "Run index_project() to build the index.",
                        });
                    }
                }
            }
        }

        // --- Memory staleness section ---
        let staleness_result = ctx
            .agent
            .with_project(|p| {
                let memories_dir = p.root.join(".codescout").join("memories");
                crate::memory::anchors::check_all_memories(&p.root, &memories_dir)
            })
            .await;
        match staleness_result {
            Ok(staleness) => {
                result["memory_staleness"] = staleness;
            }
            Err(e) => {
                tracing::debug!("memory staleness check failed: {e}");
            }
        }

        // --- Workspace section ---
        let workspace_toml_path = crate::config::workspace::workspace_config_path(&root);
        let workspace_info = if workspace_toml_path.exists() {
            std::fs::read_to_string(&workspace_toml_path)
                .ok()
                .and_then(|s| toml::from_str::<crate::config::workspace::WorkspaceConfig>(&s).ok())
                .map(|ws| {
                    json!({
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
                    })
                })
        } else {
            None
        };
        result["workspace"] = json!(workspace_info);

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_project_status(result))
    }
}

fn format_activate_project(result: &Value) -> String {
    let root = result["activated"]["project_root"]
        .as_str()
        .or_else(|| result["path"].as_str())
        .unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    if let Some(hint) = result["hint"].as_str() {
        format!("activated · {name} · {hint}")
    } else {
        format!("activated · {name}")
    }
}

fn format_project_status(result: &Value) -> String {
    let root = result["project_root"].as_str().unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    let status = result["index"]["status"].as_str().unwrap_or("unknown");
    let index_str = match status {
        "up_to_date" | "behind" => {
            let files = result["index"]["files"].as_u64().unwrap_or(0);
            let chunks = result["index"]["chunks"].as_u64().unwrap_or(0);
            format!("index:{files}f/{chunks}c ({status})")
        }
        "running" => {
            let done = result["index"]["done"].as_u64().unwrap_or(0);
            let total = result["index"]["total"].as_u64().unwrap_or(0);
            format!("index:running {done}/{total}")
        }
        _ => "index:none".to_string(),
    };
    format!("status · {name} · {index_str}")
}

/// Auto-register direct Cargo dependencies found in `~/.cargo/registry`.
/// Returns names of crates that were newly registered.
/// Never propagates errors — this is best-effort and must not block activation.
async fn auto_register_cargo_deps(
    project_root: &std::path::Path,
    ctx: &ToolContext,
) -> Vec<String> {
    let cargo_toml = project_root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return vec![];
    }

    let content = match std::fs::read_to_string(&cargo_toml) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let dep_names = parse_cargo_dep_names(&content);
    if dep_names.is_empty() {
        return vec![];
    }

    // Find the cargo registry src dir: ~/.cargo/registry/src/index.crates.io-*/
    let home = match crate::platform::home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    let registry_src = home.join(".cargo").join("registry").join("src");
    if !registry_src.exists() {
        return vec![];
    }

    // Collect index dirs (e.g. index.crates.io-6f17d22bba15001f)
    let index_dirs: Vec<_> = match std::fs::read_dir(&registry_src) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return vec![],
    };

    let mut newly_registered = vec![];

    for dep_name in &dep_names {
        // Skip if already registered
        let already = ctx
            .agent
            .with_project(|p| Ok(p.library_registry.lookup(dep_name).is_some()))
            .await
            .unwrap_or(true);
        if already {
            continue;
        }

        // Find highest-version dir matching DEPNAME-VERSION in any index dir
        // Crate dirs are named like "serde-1.0.197" — match by prefix "serde-"
        let crate_dir = find_cargo_crate_dir(&index_dirs, dep_name);
        let crate_path = match crate_dir {
            Some(p) => p,
            None => continue,
        };

        // Register it — use write lock so we can mutate the registry
        let registered: anyhow::Result<bool> = async {
            let mut inner = ctx.agent.inner.write().await;
            let project = inner
                .active_project_mut()
                .ok_or_else(|| anyhow::anyhow!("no active project"))?;
            project.library_registry.register(
                dep_name.clone(),
                crate_path.clone(),
                "rust".to_string(),
                crate::library::registry::DiscoveryMethod::Manual,
            );
            let registry_path = project.root.join(".codescout").join("libraries.json");
            project.library_registry.save(&registry_path)?;
            Ok(true)
        }
        .await;

        if registered.unwrap_or(false) {
            newly_registered.push(dep_name.clone());
        }
    }

    newly_registered
}

/// Parse direct dependency names from Cargo.toml content.
/// Handles both `name = "version"` and `name = { version = "..." }` forms.
/// Skips `[dev-dependencies]` and `[build-dependencies]`.
fn parse_cargo_dep_names(toml: &str) -> Vec<String> {
    let mut names = vec![];
    let mut in_deps = false;

    for line in toml.lines() {
        let trimmed = line.trim();

        // Section header
        if trimmed.starts_with('[') {
            // [dependencies] or [dependencies.foo] but NOT [dev-dependencies] etc.
            in_deps = trimmed == "[dependencies]"
                || trimmed.starts_with("[dependencies.")
                || trimmed.starts_with("[workspace.dependencies");
            continue;
        }

        if !in_deps {
            continue;
        }

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Extract the key (dep name) from lines like:
        //   serde = "1.0"
        //   serde = { version = "1.0", features = [...] }
        //   serde.workspace = true
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            // Strip any dotted suffix (e.g. "serde.workspace" → "serde")
            let base_name = key.split('.').next().unwrap_or(key);
            // Normalize hyphens/underscores — Cargo treats them as equivalent
            let normalized = base_name.replace('-', "_");
            if !normalized.is_empty() && !names.contains(&normalized) {
                names.push(normalized);
            }
        }
    }

    names
}

/// Find the source directory for a crate in the cargo registry index dirs.
/// Prefers the highest version when multiple versions exist.
fn find_cargo_crate_dir(
    index_dirs: &[std::path::PathBuf],
    crate_name: &str,
) -> Option<std::path::PathBuf> {
    // Cargo normalizes `-` to `_` in dir names too — search for both
    let prefix_hyphen = format!("{}-", crate_name.replace('_', "-"));
    let prefix_under = format!("{}-", crate_name.replace('-', "_"));

    let mut candidates: Vec<std::path::PathBuf> = vec![];

    for idx_dir in index_dirs {
        if let Ok(rd) = std::fs::read_dir(idx_dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                let fname = entry.file_name();
                let name = fname.to_string_lossy();
                if (name.starts_with(&prefix_hyphen) || name.starts_with(&prefix_under))
                    && entry.path().is_dir()
                {
                    candidates.push(entry.path());
                }
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Pick the lexicographically largest (roughly highest version)
    candidates.sort();
    candidates.into_iter().next_back()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
        crate::lsp::LspManager::new_arc()
    }

    #[tokio::test]
    async fn activate_and_get_config() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // No project initially
        assert!(ProjectStatus.call(json!({}), &ctx).await.is_err());

        // Activate
        let result = ActivateProject
            .call(
                json!({
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");

        // Now project_status works
        let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        assert!(status["project_root"].as_str().unwrap().len() > 0);
        assert!(status["languages"].is_array());
        assert!(status["embeddings_model"].is_string());
    }

    #[tokio::test]
    async fn activate_nonexistent_path_errors() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let result = ActivateProject
            .call(
                json!({
                    "path": "/nonexistent/path/xyz"
                }),
                &ctx,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn activate_replaces_previous_project() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let ctx = ToolContext {
            agent: Agent::new(Some(dir1.path().to_path_buf())).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // Activate dir2
        ActivateProject
            .call(
                json!({
                    "path": dir2.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let root = status["project_root"].as_str().unwrap();
        assert!(root.contains(dir2.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn project_status_returns_all_sections() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let tool = ProjectStatus;
        let result = tool.call(json!({}), &ctx).await.unwrap();
        assert!(result["project_root"].is_string(), "missing project_root");
        assert!(result["languages"].is_array(), "missing languages field");
        assert!(
            result["embeddings_model"].is_string(),
            "missing embeddings_model field"
        );
        assert!(result.get("index").is_some(), "missing index section");
        assert!(
            result.get("libraries").is_some(),
            "missing libraries section"
        );
    }

    #[tokio::test]
    async fn project_status_compact_shape() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

        // Flat config fields — no blob
        assert!(result["languages"].is_array(), "missing languages");
        assert!(
            result["embeddings_model"].is_string(),
            "missing embeddings_model"
        );
        assert!(
            result.get("config").is_none(),
            "config blob must be removed"
        );

        // Index section has status field, no drift
        assert!(
            result["index"]["status"].is_string(),
            "index.status must be present"
        );
        assert!(
            result["index"].get("drift").is_none(),
            "drift must not appear in project_status"
        );

        // Libraries section still present
        assert!(result["libraries"].is_object(), "libraries section missing");
    }

    #[tokio::test]
    async fn project_status_includes_memory_staleness() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // Create memories dir and a memory file
        let memories_dir = dir.path().join(".codescout/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(memories_dir.join("architecture.md"), "# Arch").unwrap();

        // Create anchored file and sidecar
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/server.rs"), "fn main() {}").unwrap();

        let anchors =
            crate::memory::anchors::seed_anchors(dir.path(), "Uses `src/server.rs`.").unwrap();
        crate::memory::anchors::write_anchor_file(
            &memories_dir.join("architecture.anchors.toml"),
            &anchors,
        )
        .unwrap();

        // Before change — should be fresh
        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let staleness = &result["memory_staleness"];
        assert!(staleness["stale"].as_array().unwrap().is_empty());
        assert!(staleness["fresh"]
            .as_array()
            .unwrap()
            .contains(&json!("architecture")));

        // Modify the anchored file
        std::fs::write(dir.path().join("src/server.rs"), "fn changed() {}").unwrap();

        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let staleness = &result["memory_staleness"];
        let stale = staleness["stale"].as_array().unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0]["topic"], "architecture");
        assert!(stale[0]["changed_files"]
            .as_array()
            .unwrap()
            .contains(&json!("src/server.rs")));
    }

    #[tokio::test]
    async fn activate_includes_cwd_hint() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(None).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let input = json!({ "path": dir.path().to_str().unwrap() });
        let result = ActivateProject.call(input, &ctx).await.unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(
            hint.starts_with("CWD: "),
            "hint should start with CWD: but was: {hint}"
        );
        assert!(hint.contains(dir.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn activate_hint_shows_switched_when_away_from_home() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let input = json!({ "path": dir2.path().to_str().unwrap() });
        let result = ActivateProject.call(input, &ctx).await.unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(hint.starts_with("Switched project."), "hint: {hint}");
        assert!(
            hint.contains(dir2.path().to_str().unwrap()),
            "should contain new path"
        );
        assert!(
            hint.contains(dir1.path().to_str().unwrap()),
            "should contain home path"
        );
    }

    #[tokio::test]
    async fn activate_hint_shows_returned_when_back_home() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        // Switch away
        ActivateProject
            .call(json!({ "path": dir2.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        // Return home
        let result = ActivateProject
            .call(json!({ "path": dir1.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(
            hint.starts_with("Returned to original project."),
            "hint: {hint}"
        );
        assert!(hint.contains(dir1.path().to_str().unwrap()));
    }

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
        )
        .unwrap();
        std::fs::write(
            codescout.join("project.toml"),
            "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
        )
        .unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        let result = ProjectStatus
            .call(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let ws = result.get("workspace");
        assert!(
            ws.is_some(),
            "project_status should include workspace section"
        );
        let projects = ws.unwrap().get("projects").unwrap().as_array().unwrap();
        assert_eq!(projects.len(), 2);
    }

    #[tokio::test]
    async fn activate_project_switches_focus_by_id() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create multi-project structure
        std::fs::write(root.join("build.gradle.kts"), "").unwrap();
        let mcp = root.join("mcp-server");
        std::fs::create_dir_all(&mcp).unwrap();
        std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // Initially focused on root project
        let root_path = ctx.agent.require_project_root().await.unwrap();
        assert_eq!(root_path, root.to_path_buf());

        // Switch focus to mcp-server by ID
        let result = ActivateProject
            .call(serde_json::json!({"path": "mcp-server"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");

        // Now focused on mcp-server
        let new_root = ctx.agent.require_project_root().await.unwrap();
        assert_eq!(new_root, root.join("mcp-server"));
    }

    #[tokio::test]
    async fn activate_project_unknown_id_with_no_slash_returns_error() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"test\"\n").unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // "unknown-project" has no slash and does not exist as a project ID or a path
        let result = ActivateProject
            .call(serde_json::json!({"path": "unknown-project"}), &ctx)
            .await;
        // Should fail: not a known project ID, and not a valid directory path
        assert!(
            result.is_err() || result.as_ref().unwrap().get("error").is_some(),
            "expected error or error field, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn post_compact_flushes_lsp_clients_and_returns_flushed() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };

        // post_compact=true should return flushed:true without the normal status fields
        let result = ProjectStatus
            .call(json!({"post_compact": true}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["flushed"], json!(true), "expected flushed:true");
        assert!(result["hint"].is_string(), "expected hint string");
        // Normal status fields must NOT be present in the compact-flush response
        assert!(
            result.get("project_root").is_none(),
            "post_compact response must not include project_root"
        );

        // post_compact=false (or absent) should return the normal status response
        let result = ProjectStatus
            .call(json!({"post_compact": false}), &ctx)
            .await
            .unwrap();
        assert!(
            result["project_root"].is_string(),
            "normal call must include project_root"
        );
    }

    #[test]
    fn parse_cargo_dep_names_basic() {
        let toml = r#"
[package]
name = "foo"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1"

[dev-dependencies]
tempfile = "3"

[build-dependencies]
build_script = "0.1"
"#;
        let names = parse_cargo_dep_names(toml);
        // Direct deps present
        assert!(names.contains(&"serde".to_string()));
        assert!(names.contains(&"tokio".to_string()));
        assert!(names.contains(&"anyhow".to_string()));
        // dev-deps and build-deps must NOT be included
        assert!(!names.contains(&"tempfile".to_string()));
        assert!(!names.contains(&"build_script".to_string()));
    }

    #[test]
    fn parse_cargo_dep_names_normalises_hyphens() {
        let toml = r#"
[dependencies]
my-crate = "1.0"
"#;
        let names = parse_cargo_dep_names(toml);
        assert!(
            names.contains(&"my_crate".to_string()),
            "hyphens should be normalised to underscores"
        );
    }

    #[tokio::test]
    async fn activate_project_auto_registers_cargo_dependencies() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // Write a minimal Cargo.toml with one dep that lives in a fake registry
        let fake_registry_src = dir.path().join("fake_registry");
        let fake_crate_dir = fake_registry_src.join("mylib-1.2.3");
        std::fs::create_dir_all(&fake_crate_dir).unwrap();

        // Patch HOME so auto_register_cargo_deps uses our fake registry
        // We use a custom Cargo.toml that points to a path dep to avoid needing
        // the real ~/.cargo/registry for this test. Instead we test the internal
        // helpers directly and exercise the activation path via find_cargo_crate_dir.
        let cargo_toml =
            format!("[package]\nname = \"test\"\n\n[dependencies]\nmylib = \"1.2.3\"\n");
        std::fs::write(dir.path().join("Cargo.toml"), &cargo_toml).unwrap();

        // Verify parse_cargo_dep_names picks up the dep
        let names = parse_cargo_dep_names(&cargo_toml);
        assert!(
            names.contains(&"mylib".to_string()),
            "mylib should be parsed"
        );

        // Verify find_cargo_crate_dir finds it given our fake index
        let index_dirs = vec![fake_registry_src];
        let found = find_cargo_crate_dir(&index_dirs, "mylib");
        assert_eq!(
            found.as_deref(),
            Some(fake_crate_dir.as_path()),
            "should find fake crate dir"
        );

        // Full activation: with no real ~/.cargo/registry, no libs get registered,
        // but activation must still succeed (best-effort — never blocks activation).
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
        };
        let result = ActivateProject
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["status"], "ok", "activation must succeed");
    }
}
