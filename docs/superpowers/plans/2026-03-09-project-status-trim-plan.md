# project_status Trim Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the noisy `config` blob and verbose `index` section in `project_status` with flat essential fields and a compact index summary that delegates details to `index_status`.

**Architecture:** Single file change — rewrite `ProjectStatus::call` in `src/tools/config.rs`. Extract only `languages` and `embeddings_model` from config; rebuild the `index` section as a `status` string + stats + hint; remove drift params from the schema; update `format_project_status` and affected tests.

**Tech Stack:** Rust, `serde_json`, `crate::agent::IndexingState` (already `Clone`).

---

### Task 1: Trim `project_status` — new output shape

**Files:**
- Modify: `src/tools/config.rs` (all changes in one file)

---

**Step 1: Write failing tests**

In `src/tools/config.rs`, in the `tests` module, add this new test after `project_status_returns_all_sections`:

```rust
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
    };
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    // Flat config fields — no blob
    assert!(result["languages"].is_array(), "missing languages");
    assert!(result["embeddings_model"].is_string(), "missing embeddings_model");
    assert!(result.get("config").is_none(), "config blob must be removed");

    // Index section has status field, no drift
    assert!(result["index"]["status"].is_string(), "index.status must be present");
    assert!(result["index"].get("drift").is_none(), "drift must not appear in project_status");

    // Libraries section still present
    assert!(result["libraries"].is_object(), "libraries section missing");
}
```

Also update `project_status_returns_all_sections` — replace the `config` assertion:

```rust
// OLD:
assert!(result["config"].is_object(), "missing config section");

// NEW:
assert!(result["languages"].is_array(), "missing languages field");
assert!(result["embeddings_model"].is_string(), "missing embeddings_model field");
```

**Step 2: Run tests to verify they fail**

```
cargo test project_status 2>&1 | tail -20
```

Expected: `project_status_compact_shape` fails (config blob still present); `project_status_returns_all_sections` fails (config assertion).

---

**Step 3: Replace `ProjectStatus::call`**

Use `replace_symbol` to replace the entire `call` method body. The new implementation:

```rust
async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    use crate::agent::IndexingState;

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

    if let IndexingState::Running { done, total, eta_secs } = indexing_state {
        result["index"] = json!({
            "status": "running",
            "done": done,
            "total": total,
            "eta_secs": eta_secs,
            "hint": "Call index_status() for detailed breakdown.",
        });
    } else {
        let db_path = crate::embed::index::db_path(&root);
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
                let staleness =
                    crate::embed::index::check_index_staleness(&conn, &root2).ok();
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

    Ok(result)
}
```

Note: parameter renamed from `input` to `_input` since it's no longer used (drift params removed).

---

**Step 4: Replace `input_schema` — remove drift params**

Replace the entire `input_schema` method with an empty schema (no params):

```rust
fn input_schema(&self) -> Value {
    json!({ "type": "object", "properties": {} })
}
```

---

**Step 5: Update `description`**

Replace:

```rust
fn description(&self) -> &str {
    "Active project state: config, semantic index health, usage telemetry, and library summary. \
     Pass threshold (float) to include drift scores. Pass window ('1h','24h','7d','30d') for usage window."
}
```

with:

```rust
fn description(&self) -> &str {
    "Active project state: languages, embedding model, index health summary, and memory staleness. \
     Call index_status() for detailed index info, drift scores, and live progress."
}
```

---

**Step 6: Update `format_project_status`**

Replace the body — `index.indexed` no longer exists, use `index.status`:

```rust
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
```

---

**Step 7: Run tests**

```
cargo test project_status 2>&1 | tail -20
```

Expected: all `project_status_*` tests pass.

**Step 8: Full suite + lint**

```
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -5
cargo test 2>&1 | grep "test result"
```

Expected: all pass, no warnings.

**Step 9: Commit**

```
git add src/tools/config.rs
git commit -m "feat: trim project_status — flat config fields, compact index summary"
```

---

## Verification

After `cargo build --release` + `/mcp` restart:

```json
// Expected project_status output shape:
{
  "project_root": "...",
  "languages": ["rust", ...],
  "embeddings_model": "ollama:mxbai-embed-large",
  "libraries": { "count": 0, "indexed": 0 },
  "index": {
    "status": "up_to_date",
    "files": 289,
    "chunks": 11224,
    "last_updated": "...",
    "hint": "Call index_status() for model info, by_source breakdown, drift, and progress details."
  },
  "memory_staleness": { "stale": [], "fresh": [...], "untracked": [...] }
}
```
