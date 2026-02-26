# Library Search Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable searching and navigating third-party library/dependency source code through code-explorer, read-only, via a progressive 4-level rollout.

**Architecture:** A `LibraryRegistry` tracks discovered library paths in `.code-explorer/libraries.json`. Path security is extended to allow read-only access to registered library paths. Tools gain an optional `scope` parameter to target project, libraries, or both. Embedding index gains a `source` column to distinguish project vs library chunks.

**Tech Stack:** Rust, serde_json (persistence), existing LSP/embed/path_security infrastructure.

**Design doc:** `docs/plans/2026-02-26-library-search-design.md`

---

## Phase 0: Foundation — Library Registry

### Task 1: LibraryEntry and DiscoveryMethod data model

**Files:**
- Create: `src/library/mod.rs`
- Create: `src/library/registry.rs`
- Modify: `src/main.rs` (add `mod library`)
- Test: `src/library/registry.rs` (inline tests)

**Step 1: Write the failing test**

In `src/library/registry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_entry_roundtrip_json() {
        let entry = LibraryEntry {
            name: "serde".into(),
            version: Some("1.0.210".into()),
            path: PathBuf::from("/home/user/.cargo/registry/src/serde-1.0.210"),
            language: "rust".into(),
            discovered_via: DiscoveryMethod::LspFollowThrough,
            indexed: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: LibraryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "serde");
        assert_eq!(parsed.version, Some("1.0.210".into()));
        assert!(!parsed.indexed);
    }

    #[test]
    fn discovery_method_serializes_as_string() {
        let json = serde_json::to_value(DiscoveryMethod::LspFollowThrough).unwrap();
        assert_eq!(json, serde_json::json!("lsp_follow_through"));
        let json = serde_json::to_value(DiscoveryMethod::Manual).unwrap();
        assert_eq!(json, serde_json::json!("manual"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test library::registry::tests -- --nocapture`
Expected: FAIL — module doesn't exist yet

**Step 3: Write minimal implementation**

Create `src/library/mod.rs`:

```rust
pub mod registry;
```

Create `src/library/registry.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryEntry {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub language: String,
    pub discovered_via: DiscoveryMethod,
    pub indexed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMethod {
    LspFollowThrough,
    Manual,
}
```

Add to `src/main.rs` alongside the other `mod` declarations:

```rust
mod library;
```

**Step 4: Run test to verify it passes**

Run: `cargo test library::registry::tests -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/library/ src/main.rs
git commit -m "feat(library): add LibraryEntry and DiscoveryMethod data model"
```

---

### Task 2: LibraryRegistry with persistence

**Files:**
- Modify: `src/library/registry.rs`
- Test: `src/library/registry.rs` (inline tests)

**Step 1: Write the failing tests**

Add to the `tests` module in `src/library/registry.rs`:

```rust
    #[test]
    fn register_and_lookup() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
        );
        let entry = registry.lookup("serde").unwrap();
        assert_eq!(entry.path, PathBuf::from("/tmp/serde-1.0"));
        assert!(!entry.indexed);
    }

    #[test]
    fn register_updates_existing() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
        );
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde-1.1"),
            "rust".into(),
            DiscoveryMethod::LspFollowThrough,
        );
        assert_eq!(registry.all().len(), 1);
        assert_eq!(registry.lookup("serde").unwrap().path, PathBuf::from("/tmp/serde-1.1"));
    }

    #[test]
    fn is_library_path_matches() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
        );
        let entry = registry.is_library_path(Path::new("/tmp/serde-1.0/src/de.rs"));
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "serde");

        assert!(registry.is_library_path(Path::new("/tmp/other/file.rs")).is_none());
    }

    #[test]
    fn resolve_path_works() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
        );
        let resolved = registry.resolve_path("serde", "src/de.rs").unwrap();
        assert_eq!(resolved, PathBuf::from("/tmp/serde-1.0/src/de.rs"));
        assert!(registry.resolve_path("unknown", "foo.rs").is_err());
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("libraries.json");

        let mut registry = LibraryRegistry::new();
        registry.register(
            "serde".into(),
            PathBuf::from("/tmp/serde"),
            "rust".into(),
            DiscoveryMethod::Manual,
        );
        registry.save(&json_path).unwrap();

        let loaded = LibraryRegistry::load(&json_path).unwrap();
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.lookup("serde").unwrap().name, "serde");
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let registry = LibraryRegistry::load(Path::new("/nonexistent/libraries.json")).unwrap();
        assert!(registry.all().is_empty());
    }
```

**Step 2: Run test to verify they fail**

Run: `cargo test library::registry::tests -- --nocapture`
Expected: FAIL — `LibraryRegistry` doesn't exist

**Step 3: Write minimal implementation**

Add to `src/library/registry.rs`:

```rust
use anyhow::{bail, Result};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct LibraryRegistry {
    entries: Vec<LibraryEntry>,
}

impl LibraryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let text = std::fs::read_to_string(path)?;
        let entries: Vec<LibraryEntry> = serde_json::from_str(&text)?;
        Ok(Self { entries })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn register(
        &mut self,
        name: String,
        path: PathBuf,
        language: String,
        discovered_via: DiscoveryMethod,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.name == name) {
            existing.path = path;
            existing.language = language;
            existing.discovered_via = discovered_via;
            // Preserve indexed status only if path hasn't changed
        } else {
            self.entries.push(LibraryEntry {
                name,
                version: None,
                path,
                language,
                discovered_via,
                indexed: false,
            });
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut LibraryEntry> {
        self.entries.iter_mut().find(|e| e.name == name)
    }

    pub fn all(&self) -> &[LibraryEntry] {
        &self.entries
    }

    pub fn resolve_path(&self, name: &str, relative: &str) -> Result<PathBuf> {
        let entry = self.lookup(name).ok_or_else(|| {
            anyhow::anyhow!("library '{}' not found in registry", name)
        })?;
        Ok(entry.path.join(relative))
    }

    pub fn is_library_path(&self, absolute_path: &Path) -> Option<&LibraryEntry> {
        self.entries.iter().find(|e| absolute_path.starts_with(&e.path))
    }
}
```

**Step 4: Run test to verify they pass**

Run: `cargo test library::registry::tests -- --nocapture`
Expected: PASS (all 7 tests)

**Step 5: Run full test suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/library/
git commit -m "feat(library): add LibraryRegistry with persistence"
```

---

### Task 3: Wire LibraryRegistry into Agent

**Files:**
- Modify: `src/agent.rs:20-24` (add `library_registry` to `ActiveProject`)
- Modify: `src/agent.rs:46-56` (load registry in `activate`)
- Test: `src/agent.rs` (extend existing tests)

**Step 1: Write the failing test**

Add to the `tests` module in `src/agent.rs`:

```rust
    #[tokio::test]
    async fn activate_loads_library_registry() {
        let dir = tempfile::tempdir().unwrap();
        init_project_dir(dir.path());
        let agent = Agent::new(None).await;
        agent.activate(dir.path()).await.unwrap();

        let inner = agent.inner.read().await;
        let project = inner.active_project.as_ref().unwrap();
        assert!(project.library_registry.all().is_empty());
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test agent::tests::activate_loads_library_registry -- --nocapture`
Expected: FAIL — `library_registry` field doesn't exist on `ActiveProject`

**Step 3: Write minimal implementation**

Add to `ActiveProject` in `src/agent.rs:20-24`:

```rust
pub struct ActiveProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub memory: MemoryStore,
    pub library_registry: LibraryRegistry,
}
```

Add the import at the top of `src/agent.rs`:

```rust
use crate::library::registry::LibraryRegistry;
```

In the `activate` method (`src/agent.rs:46-56`), load the registry when setting up `ActiveProject`:

```rust
let registry_path = root.join(".code-explorer").join("libraries.json");
let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
```

And include `library_registry` in the `ActiveProject` construction.

Add a helper method to `impl Agent`:

```rust
    pub async fn library_registry(&self) -> Option<LibraryRegistry> {
        self.inner.read().await.active_project.as_ref().map(|p| p.library_registry.clone())
    }

    pub async fn save_library_registry(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let project = inner.active_project.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active project"))?;
        let path = project.root.join(".code-explorer").join("libraries.json");
        project.library_registry.save(&path)
    }
```

**Step 4: Run test to verify it passes**

Run: `cargo test agent::tests::activate_loads_library_registry -- --nocapture`
Expected: PASS

**Step 5: Run full suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/agent.rs
git commit -m "feat(library): wire LibraryRegistry into Agent/ActiveProject"
```

---

## Phase A: Follow-Through Reads

### Task 4: Extend validate_read_path for registered library paths

**Files:**
- Modify: `src/util/path_security.rs:38-55` (add `library_paths` to `PathSecurityConfig`)
- Modify: `src/util/path_security.rs:155-185` (extend `validate_read_path`)
- Test: `src/util/path_security.rs` (inline tests)

**Step 1: Write the failing tests**

Add to the `tests` module in `src/util/path_security.rs`:

```rust
    #[test]
    fn read_registered_library_path_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let lib_dir = dir.path().join("serde");
        std::fs::create_dir_all(&lib_dir).unwrap();
        let lib_file = lib_dir.join("lib.rs");
        std::fs::write(&lib_file, "pub fn hello() {}").unwrap();

        let config = PathSecurityConfig {
            library_paths: vec![lib_dir.clone()],
            ..Default::default()
        };

        let result = validate_read_path(
            lib_file.to_str().unwrap(),
            Some(dir.path()),
            &config,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn read_unregistered_absolute_path_outside_project_denied() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = other.path().join("secret.rs");
        std::fs::write(&file, "secret").unwrap();

        let config = PathSecurityConfig::default(); // no library_paths

        let result = validate_read_path(
            file.to_str().unwrap(),
            Some(dir.path()),
            &config,
        );
        // Should still be allowed by existing behavior (absolute paths outside
        // project are currently allowed unless denied). This test documents
        // existing behavior — library_paths is an ADDITIONAL allowlist for when
        // we tighten absolute path policy later.
        // For now, existing tests cover this. We're testing that library_paths
        // works as a positive signal.
        assert!(result.is_ok());
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test path_security::tests::read_registered_library_path -- --nocapture`
Expected: FAIL — `library_paths` field doesn't exist

**Step 3: Write minimal implementation**

Add to `PathSecurityConfig` in `src/util/path_security.rs:38-55`:

```rust
    /// Read-only library paths (registered via LibraryRegistry).
    /// Absolute paths under these directories are allowed for reading.
    pub library_paths: Vec<PathBuf>,
```

Add to the `Default` impl at `src/util/path_security.rs:57-70`:

```rust
    library_paths: Vec::new(),
```

No changes needed to `validate_read_path` yet — absolute paths outside project are already allowed by the existing logic. The `library_paths` field is the data foundation that tools will check to tag results as `"source": "lib:<name>"`.

**Step 4: Run test to verify it passes**

Run: `cargo test path_security::tests -- --nocapture`
Expected: PASS (all existing + new tests)

**Step 5: Commit**

```bash
git add src/util/path_security.rs
git commit -m "feat(security): add library_paths to PathSecurityConfig"
```

---

### Task 5: ListLibraries tool

**Files:**
- Create: `src/tools/library.rs`
- Modify: `src/tools/mod.rs` (add `pub mod library`)
- Modify: `src/server.rs:55-105` (register ListLibraries)
- Test: `src/tools/library.rs` (inline tests)

**Step 1: Write the failing test**

In `src/tools/library.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_helpers::*;

    #[tokio::test]
    async fn list_libraries_empty() {
        let ctx = project_ctx().await;
        let tool = ListLibraries;
        let result = tool.call(json!({}), &ctx).await.unwrap();
        let libs = result["libraries"].as_array().unwrap();
        assert!(libs.is_empty());
    }

    #[tokio::test]
    async fn list_libraries_shows_registered() {
        let ctx = project_ctx().await;
        {
            let mut inner = ctx.agent.inner.write().await;
            let project = inner.active_project.as_mut().unwrap();
            project.library_registry.register(
                "serde".into(),
                PathBuf::from("/tmp/serde"),
                "rust".into(),
                crate::library::registry::DiscoveryMethod::Manual,
            );
        }
        let tool = ListLibraries;
        let result = tool.call(json!({}), &ctx).await.unwrap();
        let libs = result["libraries"].as_array().unwrap();
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0]["name"], "serde");
        assert_eq!(libs[0]["indexed"], false);
    }
}
```

Note: The test helper `project_ctx()` may need adaptation depending on what's available in `src/tools/`. Check `src/tools/semantic.rs:190-201` for the existing pattern.

**Step 2: Run test to verify it fails**

Run: `cargo test tools::library::tests -- --nocapture`
Expected: FAIL — module doesn't exist

**Step 3: Write minimal implementation**

Create `src/tools/library.rs`:

```rust
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

use super::{Tool, ToolContext};

pub struct ListLibraries;

#[async_trait::async_trait]
impl Tool for ListLibraries {
    fn name(&self) -> &str {
        "list_libraries"
    }

    fn description(&self) -> &str {
        "Show all registered libraries and their status (indexed, path, language)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _input: Value, ctx: &ToolContext) -> Result<Value> {
        let inner = ctx.agent.inner.read().await;
        let project = inner
            .active_project
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active project. Use activate_project first."))?;

        let libs: Vec<Value> = project
            .library_registry
            .all()
            .iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "version": entry.version,
                    "path": entry.path.display().to_string(),
                    "language": entry.language,
                    "discovered_via": entry.discovered_via,
                    "indexed": entry.indexed,
                })
            })
            .collect();

        Ok(json!({ "libraries": libs }))
    }
}
```

Add `pub mod library;` to `src/tools/mod.rs`.

Register in `src/server.rs` (in the `from_parts` tool vec, after the config tools):

```rust
            // Library tools
            Arc::new(ListLibraries),
```

Add the import in `src/server.rs`:

```rust
    library::ListLibraries,
```

**Step 4: Run test to verify it passes**

Run: `cargo test tools::library::tests -- --nocapture`
Expected: PASS

**Step 5: Run full suite + clippy + fmt**

Run: `cargo fmt && cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/library.rs src/tools/mod.rs src/server.rs
git commit -m "feat(library): add list_libraries tool"
```

---

### Task 6: Tag read_file results with source

**Files:**
- Modify: `src/tools/file.rs:13-79` (`impl Tool for ReadFile::call`)
- Test: `src/tools/file.rs` (inline tests)

**Step 1: Write the failing test**

Add to tests in `src/tools/file.rs` (create the test module if needed):

```rust
    #[tokio::test]
    async fn read_file_tags_library_source() {
        // Set up a project with a registered library
        let ctx = project_ctx().await;
        let lib_dir = tempfile::tempdir().unwrap();
        let lib_file = lib_dir.path().join("lib.rs");
        std::fs::write(&lib_file, "pub fn hello() {}").unwrap();

        {
            let mut inner = ctx.agent.inner.write().await;
            let project = inner.active_project.as_mut().unwrap();
            project.library_registry.register(
                "mylib".into(),
                lib_dir.path().to_path_buf(),
                "rust".into(),
                crate::library::registry::DiscoveryMethod::Manual,
            );
        }

        let tool = ReadFile;
        let result = tool
            .call(json!({ "path": lib_file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["source"], "lib:mylib");
    }

    #[tokio::test]
    async fn read_file_tags_project_source() {
        let ctx = project_ctx().await;
        let root = ctx.agent.require_project_root().await.unwrap();
        let file = root.join("test.txt");
        std::fs::write(&file, "hello").unwrap();

        let tool = ReadFile;
        let result = tool.call(json!({ "path": "test.txt" }), &ctx).await.unwrap();
        assert_eq!(result["source"], "project");
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test tools::file::tests::read_file_tags -- --nocapture`
Expected: FAIL — no `source` field in result

**Step 3: Write minimal implementation**

In `ReadFile::call` (`src/tools/file.rs`), after resolving the path, check the library registry and add the `"source"` field to every returned JSON object:

```rust
    // After resolving `resolved` path, determine source tag
    let source_tag = {
        let inner = ctx.agent.inner.read().await;
        if let Some(project) = &inner.active_project {
            if let Some(lib) = project.library_registry.is_library_path(&resolved) {
                format!("lib:{}", lib.name)
            } else {
                "project".to_string()
            }
        } else {
            "project".to_string()
        }
    };
```

Then include `"source": source_tag` in all returned `json!({})` objects (there are 3 return points in the function).

**Step 4: Run test to verify it passes**

Run: `cargo test tools::file::tests::read_file_tags -- --nocapture`
Expected: PASS

**Step 5: Run full suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat(library): tag read_file results with source (project vs lib)"
```

---

## Phase B: Symbol Navigation in Libraries

### Task 7: Scope parsing helper

**Files:**
- Create: `src/library/scope.rs`
- Modify: `src/library/mod.rs` (add `pub mod scope`)
- Test: `src/library/scope.rs` (inline tests)

**Step 1: Write the failing tests**

In `src/library/scope.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_default_is_project() {
        assert_eq!(Scope::parse(None), Scope::Project);
    }

    #[test]
    fn parse_scope_project() {
        assert_eq!(Scope::parse(Some("project")), Scope::Project);
    }

    #[test]
    fn parse_scope_libraries() {
        assert_eq!(Scope::parse(Some("libraries")), Scope::Libraries);
    }

    #[test]
    fn parse_scope_all() {
        assert_eq!(Scope::parse(Some("all")), Scope::All);
    }

    #[test]
    fn parse_scope_specific_lib() {
        assert_eq!(
            Scope::parse(Some("lib:serde")),
            Scope::Library("serde".into())
        );
    }

    #[test]
    fn includes_project() {
        assert!(Scope::Project.includes_project());
        assert!(!Scope::Libraries.includes_project());
        assert!(Scope::All.includes_project());
        assert!(!Scope::Library("serde".into()).includes_project());
    }

    #[test]
    fn includes_library() {
        assert!(!Scope::Project.includes_library("serde"));
        assert!(Scope::Libraries.includes_library("serde"));
        assert!(Scope::All.includes_library("serde"));
        assert!(Scope::Library("serde".into()).includes_library("serde"));
        assert!(!Scope::Library("tokio".into()).includes_library("serde"));
    }
}
```

**Step 2: Run test to verify they fail**

Run: `cargo test library::scope::tests -- --nocapture`
Expected: FAIL — module doesn't exist

**Step 3: Write minimal implementation**

Create `src/library/scope.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Only project code (default)
    Project,
    /// Only registered libraries
    Libraries,
    /// Project + all libraries
    All,
    /// A specific library by name
    Library(String),
}

impl Scope {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            None | Some("project") => Scope::Project,
            Some("libraries") => Scope::Libraries,
            Some("all") => Scope::All,
            Some(s) if s.starts_with("lib:") => Scope::Library(s[4..].to_string()),
            Some(_) => Scope::Project, // unknown defaults to project
        }
    }

    pub fn includes_project(&self) -> bool {
        matches!(self, Scope::Project | Scope::All)
    }

    pub fn includes_library(&self, name: &str) -> bool {
        match self {
            Scope::Libraries | Scope::All => true,
            Scope::Library(n) => n == name,
            Scope::Project => false,
        }
    }
}
```

Add `pub mod scope;` to `src/library/mod.rs`.

**Step 4: Run test to verify they pass**

Run: `cargo test library::scope::tests -- --nocapture`
Expected: PASS (all 7 tests)

**Step 5: Commit**

```bash
git add src/library/scope.rs src/library/mod.rs
git commit -m "feat(library): add Scope enum for scoped tool queries"
```

---

### Task 8: Scope parameter for find_symbol

**Files:**
- Modify: `src/tools/symbol.rs:354-533` (`impl Tool for FindSymbol`)
- Test: `src/tools/symbol.rs` (add to existing test module)

This is the pattern-setting task. The `scope` parameter is added to `input_schema` and `call`. When scope includes libraries, the tool iterates over matching library entries and runs LSP against their roots.

**Step 1: Add scope to input_schema**

In `FindSymbol::input_schema` (`src/tools/symbol.rs:354-368`), add to the properties:

```json
"scope": {
    "type": "string",
    "description": "Search scope: 'project' (default), 'libraries', 'all', or 'lib:<name>'",
    "default": "project"
}
```

**Step 2: Add scope to call**

In `FindSymbol::call` (`src/tools/symbol.rs:369-533`), parse scope from input:

```rust
let scope = crate::library::scope::Scope::parse(input["scope"].as_str());
```

When `scope.includes_project()` is true, run existing logic. When scope includes libraries, iterate over matching library entries from the registry, get/start an LSP client for each library root, and collect results with `"source": "lib:<name>"` tagging.

Add `"source": "project"` to all existing result items.

**Step 3: Write test**

```rust
    #[tokio::test]
    async fn find_symbol_default_scope_is_project() {
        let ctx = rust_project_ctx().await;
        let tool = FindSymbol;
        let result = tool.call(json!({ "pattern": "main" }), &ctx).await.unwrap();
        // All results should have source: "project"
        if let Some(symbols) = result["symbols"].as_array() {
            for sym in symbols {
                assert_eq!(sym["source"], "project");
            }
        }
    }

    #[tokio::test]
    async fn find_symbol_schema_includes_scope() {
        let tool = FindSymbol;
        let schema = tool.input_schema();
        assert!(schema["properties"]["scope"].is_object());
    }
```

**Step 4: Run tests**

Run: `cargo test tools::symbol::tests -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): add scope parameter to find_symbol"
```

---

### Task 9: Scope parameter for get_symbols_overview

**Files:**
- Modify: `src/tools/symbol.rs:178-340` (`impl Tool for GetSymbolsOverview`)
- Test: `src/tools/symbol.rs`

Same pattern as Task 8. Add `"scope"` to `input_schema`, parse in `call`, tag results. When scope targets a library, resolve the path within the library root instead of project root.

**Step 1: Add scope to schema and call**

Follow exact same pattern as Task 8 for `GetSymbolsOverview`.

**Step 2: Write test**

```rust
    #[tokio::test]
    async fn get_symbols_overview_schema_includes_scope() {
        let tool = GetSymbolsOverview;
        let schema = tool.input_schema();
        assert!(schema["properties"]["scope"].is_object());
    }
```

**Step 3: Run tests**

Run: `cargo test tools::symbol::tests -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): add scope parameter to get_symbols_overview"
```

---

### Task 10: Scope parameter for find_referencing_symbols

**Files:**
- Modify: `src/tools/symbol.rs:538-618` (`impl Tool for FindReferencingSymbols`)
- Test: `src/tools/symbol.rs`

Same pattern as Task 8.

**Step 1–4: Follow Task 8 pattern**

Add `"scope"` to schema, parse in call, tag results, write schema test.

**Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): add scope parameter to find_referencing_symbols"
```

---

### Task 11: Scope parameter for list_functions

**Files:**
- Modify: `src/tools/ast.rs` (find `ListFunctions` impl)
- Test: `src/tools/ast.rs`

Same pattern as Task 8. `list_functions` uses tree-sitter, not LSP, so the library-scoped version just needs to resolve paths within the library root and run tree-sitter on those files.

**Step 1–4: Follow Task 8 pattern**

**Step 5: Commit**

```bash
git add src/tools/ast.rs
git commit -m "feat(library): add scope parameter to list_functions"
```

---

## Phase C: Semantic Search in Libraries

### Task 12: Add source column to embedding schema

**Files:**
- Modify: `src/embed/index.rs:30-77` (`open_db` — schema)
- Modify: `src/embed/index.rs:87-110` (`insert_chunk`)
- Modify: `src/embed/schema.rs:6-21` (`CodeChunk` struct)
- Test: `src/embed/index.rs`

**Step 1: Write the failing test**

```rust
    #[test]
    fn insert_chunk_stores_source() {
        let (_dir, conn) = open_test_db();
        let chunk = CodeChunk {
            id: None,
            file_path: "a.rs".into(),
            language: "rust".into(),
            content: "fn a() {}".into(),
            start_line: 1,
            end_line: 1,
            file_hash: "abc".into(),
            source: "lib:serde".into(),
        };
        insert_chunk(&conn, &chunk, &[1.0, 0.0]).unwrap();

        let stored: String = conn
            .query_row("SELECT source FROM chunks WHERE file_path = 'a.rs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, "lib:serde");
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test embed::index::tests::insert_chunk_stores_source -- --nocapture`
Expected: FAIL — `source` field doesn't exist on `CodeChunk`

**Step 3: Write minimal implementation**

Add to `CodeChunk` in `src/embed/schema.rs`:

```rust
    pub source: String,  // "project" or "lib:<name>"
```

Update `open_db` schema in `src/embed/index.rs` — add `source TEXT NOT NULL DEFAULT 'project'` to the `chunks` table CREATE statement.

Update `insert_chunk` in `src/embed/index.rs` to include `source` in the INSERT.

Update all existing `CodeChunk` construction sites (in `build_index`) to set `source: "project".into()`.

**Step 4: Run test to verify it passes**

Run: `cargo test embed::index::tests -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/schema.rs src/embed/index.rs
git commit -m "feat(library): add source column to embedding schema"
```

---

### Task 13: Scoped search in embed/index.rs

**Files:**
- Modify: `src/embed/index.rs:151-194` (`search` function)
- Test: `src/embed/index.rs`

**Step 1: Write the failing test**

```rust
    #[test]
    fn search_with_source_filter() {
        let (_dir, conn) = open_test_db();
        let project_chunk = CodeChunk {
            source: "project".into(),
            ..dummy_chunk("a.rs", "fn project_fn() {}")
        };
        let lib_chunk = CodeChunk {
            source: "lib:serde".into(),
            ..dummy_chunk("b.rs", "fn lib_fn() {}")
        };
        insert_chunk(&conn, &project_chunk, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        insert_chunk(&conn, &lib_chunk, &[0.9, 0.1, 0.0, 0.0]).unwrap();

        // Search with source filter for project only
        let results = search_scoped(&conn, &[1.0, 0.0, 0.0, 0.0], 10, Some("project")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "a.rs");

        // Search with source filter for lib:serde
        let results = search_scoped(&conn, &[1.0, 0.0, 0.0, 0.0], 10, Some("lib:serde")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "b.rs");

        // Search with no filter (all)
        let results = search_scoped(&conn, &[1.0, 0.0, 0.0, 0.0], 10, None).unwrap();
        assert_eq!(results.len(), 2);
    }
```

**Step 2: Run test to verify it fails**

Expected: FAIL — `search_scoped` doesn't exist

**Step 3: Write minimal implementation**

Add `search_scoped` function alongside existing `search` in `src/embed/index.rs`:

```rust
pub fn search_scoped(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    source_filter: Option<&str>,
) -> Result<Vec<SearchResult>> {
    let query = match source_filter {
        Some(src) if src == "project" => {
            format!(
                "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, ce.embedding, c.source \
                 FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid \
                 WHERE c.source = '{}'", src
            )
        }
        Some(src) => {
            // "lib:serde" or "libraries" (source != 'project')
            if src == "libraries" {
                "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, ce.embedding, c.source \
                 FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid \
                 WHERE c.source != 'project'".to_string()
            } else {
                format!(
                    "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, ce.embedding, c.source \
                     FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid \
                     WHERE c.source = '{}'", src
                )
            }
        }
        None => {
            "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, ce.embedding, c.source \
             FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid".to_string()
        }
    };
    // ... cosine similarity scoring same as existing search(), with source in SearchResult
}
```

**Important:** Use parameterized queries instead of string interpolation for the source filter to avoid SQL injection. The above is pseudocode — use `conn.prepare` with `?` placeholders.

Also add `source: String` to `SearchResult` in `src/embed/schema.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test embed::index::tests::search_with_source_filter -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/embed/index.rs src/embed/schema.rs
git commit -m "feat(library): add search_scoped with source filtering"
```

---

### Task 14: IndexLibrary tool

**Files:**
- Modify: `src/tools/library.rs` (add `IndexLibrary` struct)
- Modify: `src/embed/index.rs` (add `build_library_index`)
- Modify: `src/server.rs` (register IndexLibrary)
- Test: `src/tools/library.rs`

**Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn index_library_requires_registered_lib() {
        let ctx = project_ctx().await;
        let tool = IndexLibrary;
        let result = tool.call(json!({ "name": "nonexistent" }), &ctx).await;
        assert!(result.is_err() || result.unwrap()["error"].is_string());
    }
```

**Step 2: Write minimal implementation**

Add `IndexLibrary` to `src/tools/library.rs`:

```rust
pub struct IndexLibrary;

#[async_trait::async_trait]
impl Tool for IndexLibrary {
    fn name(&self) -> &str {
        "index_library"
    }

    fn description(&self) -> &str {
        "Build embedding index for a registered library. Library must be in the registry (discovered via goto_definition or manually registered)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string", "description": "Library name (as shown in list_libraries)" },
                "force": { "type": "boolean", "description": "Re-index even if already done", "default": false }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let name = input["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'name' parameter"))?;
        let force = input["force"].as_bool().unwrap_or(false);

        let (lib_path, project_root) = {
            let inner = ctx.agent.inner.read().await;
            let project = inner.active_project.as_ref()
                .ok_or_else(|| anyhow::anyhow!("No active project"))?;
            let entry = project.library_registry.lookup(name)
                .ok_or_else(|| anyhow::anyhow!(
                    "Library '{}' not found. Use list_libraries to see registered libraries.", name
                ))?;
            (entry.path.clone(), project.root.clone())
        };

        let source = format!("lib:{}", name);
        crate::embed::index::build_library_index(&project_root, &lib_path, &source, force).await?;

        // Mark as indexed in registry
        {
            let mut inner = ctx.agent.inner.write().await;
            if let Some(project) = inner.active_project.as_mut() {
                if let Some(entry) = project.library_registry.lookup_mut(name) {
                    entry.indexed = true;
                }
            }
        }
        ctx.agent.save_library_registry().await?;

        let conn = crate::embed::index::open_db(&project_root)?;
        let stats = crate::embed::index::index_stats(&conn)?;

        Ok(json!({
            "status": "ok",
            "library": name,
            "files_indexed": stats.file_count,
            "chunks": stats.chunk_count,
        }))
    }
}
```

Add `build_library_index` to `src/embed/index.rs` — a variant of `build_index` that:
- Walks `lib_path` instead of `project_root`
- Uses `project_root` for the DB location (same `embeddings.db`)
- Sets `source` on all `CodeChunk` instances to the provided source string

Register in `src/server.rs`.

**Step 3: Run test + full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add src/tools/library.rs src/embed/index.rs src/server.rs
git commit -m "feat(library): add index_library tool and build_library_index"
```

---

### Task 15: Scope parameter for semantic_search

**Files:**
- Modify: `src/tools/semantic.rs:18-100` (`impl Tool for SemanticSearch`)
- Test: `src/tools/semantic.rs`

**Step 1: Add scope to schema**

Add `"scope"` property to `SemanticSearch::input_schema` (same description as Task 8).

**Step 2: Use search_scoped in call**

In `SemanticSearch::call` (`src/tools/semantic.rs:39-100`), parse scope:

```rust
let scope = crate::library::scope::Scope::parse(input["scope"].as_str());
let source_filter = match &scope {
    Scope::Project => Some("project"),
    Scope::Library(name) => Some(format!("lib:{}", name).as_str()),
    Scope::Libraries => Some("libraries"),
    Scope::All => None,
};
```

Replace `crate::embed::index::search(...)` with `crate::embed::index::search_scoped(...)`.

Add `"source"` field to each result item from `r.source`.

**Step 3: Write test**

```rust
    #[tokio::test]
    async fn semantic_search_schema_has_scope() {
        let tool = SemanticSearch;
        let schema = tool.input_schema();
        assert!(schema["properties"]["scope"].is_object());
    }
```

**Step 4: Run tests + commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(library): add scope parameter to semantic_search"
```

---

### Task 16: Extended index_status with per-source breakdown

**Files:**
- Modify: `src/embed/index.rs` (add `index_stats_by_source`)
- Modify: `src/tools/semantic.rs:138-178` (`impl Tool for IndexStatus`)
- Test: both files

**Step 1: Write the failing test**

In `src/embed/index.rs` tests:

```rust
    #[test]
    fn index_stats_by_source_groups_correctly() {
        let (_dir, conn) = open_test_db();
        let p = CodeChunk { source: "project".into(), ..dummy_chunk("a.rs", "fn a()") };
        let l = CodeChunk { source: "lib:serde".into(), ..dummy_chunk("b.rs", "fn b()") };
        insert_chunk(&conn, &p, &[1.0, 0.0]).unwrap();
        insert_chunk(&conn, &l, &[0.0, 1.0]).unwrap();

        let stats = index_stats_by_source(&conn).unwrap();
        assert_eq!(stats.len(), 2);
        assert!(stats.contains_key("project"));
        assert!(stats.contains_key("lib:serde"));
    }
```

**Step 2: Implement**

Add `index_stats_by_source` to `src/embed/index.rs`:

```rust
pub fn index_stats_by_source(conn: &Connection) -> Result<HashMap<String, IndexStats>> {
    let mut stmt = conn.prepare(
        "SELECT source, COUNT(DISTINCT file_path), COUNT(*) FROM chunks GROUP BY source"
    )?;
    // ... collect into HashMap
}
```

Update `IndexStatus::call` to include the per-source breakdown in its output.

**Step 3: Run tests + commit**

```bash
git add src/embed/index.rs src/tools/semantic.rs
git commit -m "feat(library): extend index_status with per-source breakdown"
```

---

## Phase D: LSP-Inferred Discovery

### Task 17: Manifest parsing for auto-discovery

**Files:**
- Create: `src/library/discovery.rs`
- Modify: `src/library/mod.rs` (add `pub mod discovery`)
- Test: `src/library/discovery.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_from_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[package]
name = "serde"
version = "1.0.210"
"#,
        ).unwrap();

        let result = discover_library_root(dir.path().join("src/de.rs").as_path()).unwrap();
        assert_eq!(result.name, "serde");
        assert_eq!(result.version, Some("1.0.210".into()));
        assert_eq!(result.path, dir.path().to_path_buf());
        assert_eq!(result.language, "rust");
    }

    #[test]
    fn discover_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "name": "lodash", "version": "4.17.21" }"#,
        ).unwrap();

        let result = discover_library_root(dir.path().join("index.js").as_path()).unwrap();
        assert_eq!(result.name, "lodash");
        assert_eq!(result.version, Some("4.17.21".into()));
        assert_eq!(result.language, "javascript");
    }

    #[test]
    fn discover_from_pyproject_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"[project]
name = "requests"
version = "2.31.0"
"#,
        ).unwrap();

        let result = discover_library_root(dir.path().join("src/requests/api.py").as_path()).unwrap();
        assert_eq!(result.name, "requests");
    }

    #[test]
    fn discover_fallback_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        // No manifest file — fallback to directory name
        let result = discover_library_root(dir.path().join("lib.rs").as_path());
        assert!(result.is_some());
    }

    #[test]
    fn discover_walks_up_parents() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("src").join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[package]
name = "deep_crate"
version = "0.1.0"
"#,
        ).unwrap();

        let result = discover_library_root(nested.join("mod.rs").as_path()).unwrap();
        assert_eq!(result.name, "deep_crate");
        assert_eq!(result.path, dir.path().to_path_buf());
    }
}
```

**Step 2: Run test to verify they fail**

Run: `cargo test library::discovery::tests -- --nocapture`
Expected: FAIL — module doesn't exist

**Step 3: Write minimal implementation**

Create `src/library/discovery.rs`:

```rust
use std::path::{Path, PathBuf};

pub struct DiscoveredLibrary {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub language: String,
}

/// Walk up from a file path to find a package manifest and extract metadata.
pub fn discover_library_root(file_path: &Path) -> Option<DiscoveredLibrary> {
    let mut dir = file_path.parent()?;

    loop {
        // Check each manifest type
        if let Some(result) = try_cargo_toml(dir) {
            return Some(result);
        }
        if let Some(result) = try_package_json(dir) {
            return Some(result);
        }
        if let Some(result) = try_pyproject_toml(dir) {
            return Some(result);
        }
        if let Some(result) = try_go_mod(dir) {
            return Some(result);
        }

        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }

    // Fallback: use the deepest directory name
    let fallback_dir = file_path.parent()?;
    Some(DiscoveredLibrary {
        name: fallback_dir.file_name()?.to_string_lossy().into_owned(),
        version: None,
        path: fallback_dir.to_path_buf(),
        language: "unknown".into(),
    })
}

fn try_cargo_toml(dir: &Path) -> Option<DiscoveredLibrary> {
    let manifest = dir.join("Cargo.toml");
    if !manifest.exists() { return None; }
    let text = std::fs::read_to_string(&manifest).ok()?;

    let name = extract_toml_value(&text, "name")?;
    let version = extract_toml_value(&text, "version");

    Some(DiscoveredLibrary {
        name,
        version,
        path: dir.to_path_buf(),
        language: "rust".into(),
    })
}

fn try_package_json(dir: &Path) -> Option<DiscoveredLibrary> {
    let manifest = dir.join("package.json");
    if !manifest.exists() { return None; }
    let text = std::fs::read_to_string(&manifest).ok()?;

    let name = extract_json_value(&text, "name")?;
    let version = extract_json_value(&text, "version");

    Some(DiscoveredLibrary {
        name,
        version,
        path: dir.to_path_buf(),
        language: "javascript".into(),
    })
}

fn try_pyproject_toml(dir: &Path) -> Option<DiscoveredLibrary> {
    let manifest = dir.join("pyproject.toml");
    if !manifest.exists() { return None; }
    let text = std::fs::read_to_string(&manifest).ok()?;

    let name = extract_toml_value(&text, "name")?;
    let version = extract_toml_value(&text, "version");

    Some(DiscoveredLibrary {
        name,
        version,
        path: dir.to_path_buf(),
        language: "python".into(),
    })
}

fn try_go_mod(dir: &Path) -> Option<DiscoveredLibrary> {
    let manifest = dir.join("go.mod");
    if !manifest.exists() { return None; }
    let text = std::fs::read_to_string(&manifest).ok()?;

    // "module github.com/user/repo"
    let name = text.lines()
        .find(|l| l.starts_with("module "))
        .map(|l| l.trim_start_matches("module ").trim().to_string())?;

    Some(DiscoveredLibrary {
        name,
        version: None,
        path: dir.to_path_buf(),
        language: "go".into(),
    })
}

/// Best-effort: extract `key = "value"` from TOML
fn extract_toml_value(text: &str, key: &str) -> Option<String> {
    let pattern = format!("{} = \"", key);
    text.lines()
        .find(|l| l.trim().starts_with(&pattern))
        .and_then(|l| {
            let start = l.find('"')? + 1;
            let end = l[start..].find('"')? + start;
            Some(l[start..end].to_string())
        })
}

/// Best-effort: extract `"key": "value"` from JSON
fn extract_json_value(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    text.lines()
        .find(|l| l.contains(&pattern))
        .and_then(|l| {
            let after_key = l.find(&pattern)? + pattern.len();
            let rest = &l[after_key..];
            let start = rest.find('"')? + 1;
            let end = rest[start..].find('"')? + start;
            Some(rest[start..end].to_string())
        })
}
```

Add `pub mod discovery;` to `src/library/mod.rs`.

**Step 4: Run test to verify they pass**

Run: `cargo test library::discovery::tests -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/library/discovery.rs src/library/mod.rs
git commit -m "feat(library): add manifest-based library discovery"
```

---

### Task 18: Auto-discovery from goto_definition

**Files:**
- Modify: `src/tools/symbol.rs` (in `FindSymbol::call`, where goto_definition results are processed)
- Test: `src/tools/symbol.rs`

This is the integration point where Levels A and D come together. When `FindSymbol` (or any tool that calls `goto_definition`) gets a result outside project root, it triggers discovery and registration.

**Step 1: Write helper function**

Add to `src/tools/symbol.rs` (or `src/library/registry.rs`):

```rust
/// Check if a path is outside the project root. If so, attempt to discover
/// and register the library. Returns the source tag.
async fn tag_external_path(
    path: &Path,
    project_root: &Path,
    agent: &Agent,
) -> String {
    if path.starts_with(project_root) {
        return "project".to_string();
    }

    // Check if already registered
    if let Some(registry) = agent.library_registry().await {
        if let Some(entry) = registry.is_library_path(path) {
            return format!("lib:{}", entry.name);
        }
    }

    // Attempt auto-discovery
    if let Some(discovered) = crate::library::discovery::discover_library_root(path) {
        let name = discovered.name.clone();
        let mut inner = agent.inner.write().await;
        if let Some(project) = inner.active_project.as_mut() {
            project.library_registry.register(
                discovered.name,
                discovered.path,
                discovered.language,
                crate::library::registry::DiscoveryMethod::LspFollowThrough,
            );
            // Best-effort save — don't fail the tool call if this fails
            let registry_path = project.root.join(".code-explorer").join("libraries.json");
            let _ = project.library_registry.save(&registry_path);
        }
        format!("lib:{}", name)
    } else {
        "external".to_string()
    }
}
```

**Step 2: Wire into FindSymbol::call**

In the section of `FindSymbol::call` where `goto_definition` results are processed, use `tag_external_path` to tag each location and auto-register unknown libraries.

**Step 3: Write test**

This requires an integration test with a real LSP that returns external paths — may need to be an ignored test or a mock. At minimum, test the `tag_external_path` helper:

```rust
    #[tokio::test]
    async fn tag_external_path_registers_unknown_library() {
        let ctx = rust_project_ctx().await;
        let root = ctx.agent.require_project_root().await.unwrap();

        // Create a fake library directory with Cargo.toml
        let lib_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            lib_dir.path().join("Cargo.toml"),
            "[package]\nname = \"fake_lib\"\nversion = \"0.1.0\"\n",
        ).unwrap();
        let lib_file = lib_dir.path().join("src").join("lib.rs");
        std::fs::create_dir_all(lib_file.parent().unwrap()).unwrap();
        std::fs::write(&lib_file, "pub fn hello() {}").unwrap();

        let tag = tag_external_path(&lib_file, &root, &ctx.agent).await;
        assert_eq!(tag, "lib:fake_lib");

        // Verify it was registered
        let registry = ctx.agent.library_registry().await.unwrap();
        assert!(registry.lookup("fake_lib").is_some());
    }
```

**Step 4: Run tests + full suite**

Run: `cargo fmt && cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 5: Commit**

```bash
git add src/tools/symbol.rs src/library/
git commit -m "feat(library): auto-discover libraries from goto_definition results"
```

---

## Final: Verification

### Task 19: Full integration verification

**Step 1: Run full test suite**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Expected: All tests pass, no warnings.

**Step 2: Manual smoke test**

```bash
cargo run -- start --project .
```

In an MCP client, verify:
1. `list_libraries` returns empty list
2. `find_symbol("LspClient", scope: "project")` works
3. Result items include `"source": "project"`

**Step 3: Update server instructions**

Modify `src/prompts/server_instructions.md` to document the new `scope` parameter, `list_libraries`, and `index_library` tools in the routing guidance.

**Step 4: Final commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: update server instructions with library search tools"
```

---

## Task Summary

| Task | Phase | Description | Key files |
|------|-------|-------------|-----------|
| 1 | 0 | LibraryEntry + DiscoveryMethod model | `src/library/registry.rs` |
| 2 | 0 | LibraryRegistry with persistence | `src/library/registry.rs` |
| 3 | 0 | Wire registry into Agent | `src/agent.rs` |
| 4 | A | Extend path security for library paths | `src/util/path_security.rs` |
| 5 | A | ListLibraries tool | `src/tools/library.rs` |
| 6 | A | Tag read_file with source | `src/tools/file.rs` |
| 7 | B | Scope parsing helper | `src/library/scope.rs` |
| 8 | B | Scope for find_symbol | `src/tools/symbol.rs` |
| 9 | B | Scope for get_symbols_overview | `src/tools/symbol.rs` |
| 10 | B | Scope for find_referencing_symbols | `src/tools/symbol.rs` |
| 11 | B | Scope for list_functions | `src/tools/ast.rs` |
| 12 | C | Source column in embedding schema | `src/embed/index.rs`, `src/embed/schema.rs` |
| 13 | C | Scoped search function | `src/embed/index.rs` |
| 14 | C | IndexLibrary tool | `src/tools/library.rs`, `src/embed/index.rs` |
| 15 | C | Scope for semantic_search | `src/tools/semantic.rs` |
| 16 | C | Extended index_status | `src/embed/index.rs`, `src/tools/semantic.rs` |
| 17 | D | Manifest parsing | `src/library/discovery.rs` |
| 18 | D | Auto-discovery from goto_definition | `src/tools/symbol.rs` |
| 19 | — | Full integration verification | all |
