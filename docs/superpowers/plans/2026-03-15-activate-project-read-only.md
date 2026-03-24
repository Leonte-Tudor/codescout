# activate_project Read-Only Default — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Non-home projects activate in read-only mode by default, preventing accidental writes to repos the LLM is just browsing.

**Architecture:** Add `read_only: bool` to `ActiveProject`, wire it through `security_config()` to disable `file_write_enabled`, and accept an optional `read_only` param in the `ActivateProject` tool. The existing `check_tool_access` gate handles enforcement — no changes needed there.

**Tech Stack:** Rust, serde_json, async_trait

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/agent.rs:89-99` | Add `read_only` field to `ActiveProject` |
| Modify | `src/agent.rs:194-254` | Accept `read_only` param in `Agent::activate()` |
| Modify | `src/agent.rs:449-464` | Wire `read_only` into `security_config()` |
| Modify | `src/tools/config.rs:19-27` | Add `read_only` to input schema |
| Modify | `src/tools/config.rs:28-130` | Pass `read_only` through, update response/hint |
| Modify | `src/util/path_security.rs:332-339` | Update error message for read-only context |
| Test | `src/agent.rs` (tests module) | Unit tests for read-only behavior |

---

### Task 1: Add `read_only` field to `ActiveProject`

**Files:**
- Modify: `src/agent.rs:88-99` (`ActiveProject` struct)
- Modify: `src/agent.rs:123-130` (`Agent::new` — `ActiveProject` construction)
- Modify: `src/agent.rs:205-213` (`Agent::activate` — `ActiveProject` construction)

- [ ] **Step 1: Add the field to `ActiveProject`**

In `src/agent.rs`, add `pub read_only: bool` to the struct:

```rust
pub struct ActiveProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub memory: MemoryStore,
    pub private_memory: MemoryStore,
    pub library_registry: LibraryRegistry,
    pub dirty_files: Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>,
    pub read_only: bool,  // NEW
}
```

- [ ] **Step 2: Set `read_only: false` in `Agent::new`**

In the `ActiveProject` construction inside `Agent::new` (~line 123), add:

```rust
read_only: false,
```

This is the initial project from `--project` or CWD — always writable.

- [ ] **Step 3: Set `read_only: false` in `Agent::activate` (temporary)**

In `Agent::activate` (~line 205), add `read_only: false` to the `ActiveProject` construction. We'll make this dynamic in Task 3.

- [ ] **Step 4: Build and verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 5: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all existing tests pass

---

### Task 2: Wire `read_only` into `security_config()`

**Files:**
- Modify: `src/agent.rs:449-464` (`Agent::security_config`)
- Modify: `src/util/path_security.rs:332-339` (`check_tool_access` error message)

- [ ] **Step 1: Write failing test — read-only project blocks writes**

Add to the `tests` module in `src/agent.rs`:

```rust
#[tokio::test]
async fn read_only_project_disables_file_write() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

    // Home project: writes enabled
    let config = agent.security_config().await;
    assert!(config.file_write_enabled, "home project should be writable");

    // Manually set read_only on active project
    {
        let inner = agent.inner.read().await;
        if let Some(ws) = &inner.workspace {
            if let Some(p) = ws.active_project() {
                let mut dirty = p.dirty_files.lock().unwrap();
                drop(dirty);
            }
        }
    }
    // We need to test via activate with read_only — that comes in Task 3.
    // For now, test the security_config wiring directly by setting the field.
}
```

Actually, a cleaner approach — test `security_config` after we wire `activate` with `read_only` in Task 3. Let's wire the plumbing first.

- [ ] **Step 1 (revised): Add read_only check to `security_config`**

In `Agent::security_config()` (~line 449-464), after building `config` from `p.config.security.to_path_security_config()`, add:

```rust
if p.read_only {
    config.file_write_enabled = false;
}
```

The full method becomes:

```rust
pub async fn security_config(&self) -> crate::util::path_security::PathSecurityConfig {
    let inner = self.inner.read().await;
    match inner.active_project() {
        Some(p) => {
            let mut config = p.config.security.to_path_security_config();
            config.library_paths = p
                .library_registry
                .all()
                .iter()
                .map(|e| e.path.clone())
                .collect();
            if p.read_only {
                config.file_write_enabled = false;
            }
            config
        }
        None => crate::util::path_security::PathSecurityConfig::default(),
    }
}
```

- [ ] **Step 2: Update `check_tool_access` error message for read-only context**

In `src/util/path_security.rs`, the current error for disabled writes says:
`"File write tools are disabled. Set security.file_write_enabled = true in .codescout/project.toml to enable."`

This message is wrong when the cause is read-only mode (not a config setting). Update it to:

```rust
"create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
| "remove_symbol" | "register_library" => {
    if !config.file_write_enabled {
        bail!(
            "File writes are disabled for this project. If this project was activated \
             in read-only mode, call activate_project with read_only: false to enable writes."
        );
    }
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all tests pass (the `check_tool_access_error_message_includes_config_hint` test will need updating since we changed the message — update the assertion to match the new text)

---

### Task 3: Accept `read_only` param in `Agent::activate()`

**Files:**
- Modify: `src/agent.rs:194-254` (`Agent::activate`)

- [ ] **Step 1: Add `read_only` parameter to `activate()`**

Change signature from:
```rust
pub async fn activate(&self, root: PathBuf) -> Result<()>
```
to:
```rust
pub async fn activate(&self, root: PathBuf, read_only: Option<bool>) -> Result<()>
```

- [ ] **Step 2: Implement the read-only logic**

In the body, after `let active = ActiveProject { ... }`, compute `read_only`:

```rust
let is_home = {
    let inner = self.inner.read().await;
    inner.home_root.as_ref().map(|h| *h == root).unwrap_or(true)
};

let effective_read_only = match read_only {
    Some(false) => false,   // explicit opt-in to writes
    _ if is_home => false,  // home project always writable
    _ => true,              // non-home defaults to read-only
};

let active = ActiveProject {
    root: root.clone(),
    config,
    memory,
    private_memory,
    library_registry,
    dirty_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
    read_only: effective_read_only,
};
```

Note: the `is_home` check must happen before `activate()` sets `home_root`, since `home_root` is set at the end of the method. But looking at the code, `home_root` is set inside the `inner` write lock AFTER the workspace is built. The first call to `activate()` will have `home_root == None` — in that case, `is_home` should be `true` (first project is always home, always writable). The `unwrap_or(true)` handles this.

- [ ] **Step 3: Fix all call sites**

Search for `.activate(` calls and add the `None` parameter:

1. `ActivateProject::call` in `src/tools/config.rs` — will be updated in Task 4
2. Any test calls in `src/agent.rs` — update to `agent.activate(path, None).await`

Run: `cargo build 2>&1 | grep "error"` to find all call sites that need updating.

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles

---

### Task 4: Update `ActivateProject` tool — schema, passthrough, hints

**Files:**
- Modify: `src/tools/config.rs:19-27` (input_schema)
- Modify: `src/tools/config.rs:28-130` (call)

- [ ] **Step 1: Add `read_only` to input schema**

Update `input_schema()`:

```rust
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
```

- [ ] **Step 2: Extract and pass `read_only` in the absolute-path branch**

In the `call()` method, in the absolute-path branch (after `let root = PathBuf::from(path)`), extract the param and pass it:

```rust
let read_only = input.get("read_only").and_then(|v| v.as_bool());
ctx.agent.activate(root, read_only).await?;
```

- [ ] **Step 3: Add read-only status and hint to the response**

After `activate()`, query the read-only state and include it in the response. After building the existing `hint` string, append the read-only notice:

```rust
let is_read_only = ctx.agent.with_project(|p| Ok(p.read_only)).await.unwrap_or(false);

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
        " This project is activated in read-only mode. To enable writes, call activate_project with read_only: false."
    } else {
        ""
    };
    format!(
        "Switched project. CWD: {} — ⚠ remember to activate_project(\"{}\") \
         when done (server state is shared with parent conversation).{}",
        project_root_str, home_str, ro_notice,
    )
};

// Include read_only in the response body
Ok(json!({ "status": "ok", "activated": config, "read_only": is_read_only, "hint": hint }))
```

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles

---

### Task 5: Tests

**Files:**
- Modify: `src/agent.rs` (tests module)

- [ ] **Step 1: Test — non-home project defaults to read-only**

```rust
#[tokio::test]
async fn activate_non_home_defaults_to_read_only() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    agent.activate(dir2.path().to_path_buf(), None).await.unwrap();

    let config = agent.security_config().await;
    assert!(!config.file_write_enabled, "non-home project should be read-only by default");
}
```

- [ ] **Step 2: Run test, verify it passes**

Run: `cargo test activate_non_home_defaults_to_read_only -- --nocapture 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 3: Test — explicit `read_only: false` overrides default**

```rust
#[tokio::test]
async fn activate_non_home_with_read_only_false_is_writable() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    agent.activate(dir2.path().to_path_buf(), Some(false)).await.unwrap();

    let config = agent.security_config().await;
    assert!(config.file_write_enabled, "explicit read_only=false should enable writes");
}
```

- [ ] **Step 4: Test — returning home restores writable**

```rust
#[tokio::test]
async fn activate_home_always_writable() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();

    // Switch away (read-only)
    agent.activate(dir2.path().to_path_buf(), None).await.unwrap();
    assert!(!agent.security_config().await.file_write_enabled);

    // Return home
    agent.activate(dir1.path().to_path_buf(), None).await.unwrap();
    assert!(agent.security_config().await.file_write_enabled, "home project should always be writable");
}
```

- [ ] **Step 5: Test — first activation (no home yet) is writable**

```rust
#[tokio::test]
async fn first_activate_is_writable() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

    let agent = Agent::new(None).await.unwrap();
    agent.activate(dir.path().to_path_buf(), None).await.unwrap();

    let config = agent.security_config().await;
    assert!(config.file_write_enabled, "first activated project should be writable (becomes home)");
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass

---

### Task 6: Final verification

- [ ] **Step 1: Format and lint**

Run: `cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -10`
Expected: clean

- [ ] **Step 2: Full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add src/agent.rs src/tools/config.rs src/util/path_security.rs
git commit -m "feat(activate_project): default non-home projects to read-only

Projects activated via activate_project now default to read_only: true
when the target differs from the home project (the initial --project or
CWD). This prevents accidental writes to repos the LLM is only browsing.

Pass read_only: false explicitly to enable writes on non-home projects.
Returning to the home project always restores writable mode."
```
