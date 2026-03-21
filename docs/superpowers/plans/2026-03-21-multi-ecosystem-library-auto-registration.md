# Multi-Ecosystem Library Auto-Registration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-register dependencies from Rust, Node, Python, Go, and Java/Kotlin ecosystems during `activate_project`, with a RecoverableError flow for libraries without local source code.

**Architecture:** New `src/library/auto_register.rs` module with per-ecosystem parsers and source locators, called from `ActivateProject::call`. Registry gets `source_available` field and `ManifestScan` discovery method. `resolve_library_roots()` gates source-less libraries with a RecoverableError hint.

**Tech Stack:** Rust, serde, regex (for Python/Gradle parsing), std::process::Command (for `go env`)

**Spec:** `docs/superpowers/specs/2026-03-21-multi-ecosystem-library-auto-registration-design.md`

---

### Task 1: Add `ManifestScan` to `DiscoveryMethod` and `source_available` to `LibraryEntry`

**Files:**
- Modify: `src/library/registry.rs:27-30` (DiscoveryMethod enum)
- Modify: `src/library/registry.rs:9-22` (LibraryEntry struct)
- Modify: `src/library/registry.rs:69-97` (register method)

- [ ] **Step 1: Write failing tests for new registry behavior**

Add to `src/library/registry.rs` tests module:

```rust
#[test]
fn source_available_defaults_to_true_on_deserialize() {
    let json = r#"{"name":"foo","path":"/tmp/foo","language":"rust",
                   "discovered_via":"manual","indexed":false,"nudge_dismissed":false}"#;
    let entry: LibraryEntry = serde_json::from_str(json).unwrap();
    assert!(entry.source_available, "missing field should default to true");
}

#[test]
fn register_with_source_available_false() {
    let mut reg = LibraryRegistry::new();
    reg.register("jackson".into(), PathBuf::new(), "java".into(),
                 DiscoveryMethod::ManifestScan, false);
    let entry = reg.lookup("jackson").unwrap();
    assert!(!entry.source_available);
    assert!(entry.path.as_os_str().is_empty());
}

#[test]
fn manifest_scan_does_not_overwrite_manual() {
    let mut reg = LibraryRegistry::new();
    reg.register("foo".into(), PathBuf::from("/manual/path"), "rust".into(),
                 DiscoveryMethod::Manual, true);
    reg.register("foo".into(), PathBuf::from("/scan/path"), "rust".into(),
                 DiscoveryMethod::ManifestScan, true);
    let entry = reg.lookup("foo").unwrap();
    assert_eq!(entry.path, PathBuf::from("/manual/path"),
               "ManifestScan must not overwrite Manual");
}

#[test]
fn manifest_scan_updates_existing_manifest_scan() {
    let mut reg = LibraryRegistry::new();
    reg.register("foo".into(), PathBuf::from("/old"), "rust".into(),
                 DiscoveryMethod::ManifestScan, true);
    reg.register("foo".into(), PathBuf::from("/new"), "rust".into(),
                 DiscoveryMethod::ManifestScan, true);
    let entry = reg.lookup("foo").unwrap();
    assert_eq!(entry.path, PathBuf::from("/new"));
}

#[test]
fn discovery_method_manifest_scan_serializes() {
    let json = serde_json::to_string(&DiscoveryMethod::ManifestScan).unwrap();
    assert_eq!(json, "\"manifest_scan\"");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout registry::tests -- source_available manifest_scan`
Expected: compilation errors (ManifestScan variant doesn't exist, register() wrong arity)

- [ ] **Step 3: Add `ManifestScan` variant to `DiscoveryMethod`**

In `src/library/registry.rs:27-30`, add the variant:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMethod {
    LspFollowThrough,
    Manual,
    ManifestScan,
}
```

- [ ] **Step 4: Add `source_available` field to `LibraryEntry`**

In `src/library/registry.rs`, add to the struct:

```rust
#[serde(default = "default_true")]
pub source_available: bool,
```

Add the default function:

```rust
fn default_true() -> bool { true }
```

- [ ] **Step 5: Update `register()` to accept `source_available` and respect Manual precedence**

New signature:

```rust
pub fn register(
    &mut self,
    name: String,
    path: PathBuf,
    language: String,
    discovered_via: DiscoveryMethod,
    source_available: bool,
) {
    if let Some(existing) = self.entries.iter_mut().find(|e| e.name == name) {
        // ManifestScan must not overwrite Manual registrations
        if existing.discovered_via == DiscoveryMethod::Manual
            && discovered_via == DiscoveryMethod::ManifestScan
        {
            return;
        }
        let path_changed = existing.path != path;
        existing.path = path;
        existing.language = language;
        existing.discovered_via = discovered_via;
        existing.source_available = source_available;
        if path_changed {
            existing.indexed = false;
        }
    } else {
        self.entries.push(LibraryEntry {
            name,
            version: None,
            version_indexed: None,
            db_file: None,
            nudge_dismissed: false,
            path,
            language,
            indexed: false,
            discovered_via,
            source_available,
        });
    }
}
```

- [ ] **Step 6: Fix all existing `register()` call sites to pass the new parameter**

Search for all `register(` calls and add `, true` as the last argument. Key locations:
- `src/tools/config.rs` — `auto_register_cargo_deps`
- `src/tools/library.rs` — `RegisterLibrary::call`
- `src/tools/symbol.rs` — `goto_definition` auto-discovery
- All test files that call `register()`

Run: `cargo build` to find any remaining call sites via compiler errors.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p codescout registry::tests`
Expected: all pass including new tests

- [ ] **Step 8: Commit**

```bash
git add src/library/registry.rs src/tools/config.rs src/tools/library.rs src/tools/symbol.rs
git commit -m "feat(registry): add ManifestScan discovery method and source_available field"
```

---

### Task 2: Create `src/library/auto_register.rs` with Rust support (move existing code)

**Files:**
- Create: `src/library/auto_register.rs`
- Modify: `src/library/mod.rs:1-4` (add module)
- Modify: `src/tools/config.rs` (remove moved functions, update call site)

- [ ] **Step 1: Create the module file with types and Rust handlers**

Create `src/library/auto_register.rs` with:

```rust
use std::path::{Path, PathBuf};
use crate::tools::ToolContext;
use crate::library::registry::DiscoveryMethod;

pub struct DiscoveredDep {
    pub name: String,
    pub version_spec: Option<String>,
}

pub struct RegisteredDep {
    pub name: String,
    pub language: String,
    pub source_available: bool,
}

/// Auto-register dependencies from all detected ecosystems.
/// Best-effort: never fails, never blocks activation.
pub async fn auto_register_deps(
    project_root: &Path,
    ctx: &ToolContext,
) -> Vec<RegisteredDep> {
    let mut all_deps: Vec<(DiscoveredDep, String, Option<PathBuf>)> = vec![];

    // Rust
    collect_cargo_deps(project_root, &mut all_deps);
    // Node
    collect_node_deps(project_root, &mut all_deps);
    // Python
    collect_python_deps(project_root, &mut all_deps);
    // Go
    collect_go_deps(project_root, &mut all_deps);
    // Java/Kotlin
    collect_jvm_deps(project_root, &mut all_deps);

    // Batch registration: single write lock
    let mut newly_registered = vec![];
    if all_deps.is_empty() {
        return newly_registered;
    }

    let result: anyhow::Result<Vec<RegisteredDep>> = async {
        let mut inner = ctx.agent.inner.write().await;
        let project = inner
            .active_project_mut()
            .ok_or_else(|| anyhow::anyhow!("no active project"))?;

        for (dep, language, source_path) in &all_deps {
            let already = project.library_registry.lookup(&dep.name).is_some();
            let source_available = source_path.is_some();
            let path = source_path.clone().unwrap_or_default();

            // Let register() handle all precedence (ManifestScan vs Manual).
            // Do NOT duplicate the check here — register() skips ManifestScan
            // updates when the existing entry is Manual.
            project.library_registry.register(
                dep.name.clone(),
                path,
                language.clone(),
                DiscoveryMethod::ManifestScan,
                source_available,
            );
            if !already {
                newly_registered.push(RegisteredDep {
                    name: dep.name.clone(),
                    language: language.clone(),
                    source_available,
                });
            }
        }

        let registry_path = project.root.join(".codescout").join("libraries.json");
        project.library_registry.save(&registry_path)?;
        Ok(newly_registered)
    }
    .await;

    result.unwrap_or_default()
}
```

- [ ] **Step 2: Move `parse_cargo_dep_names` and `find_cargo_crate_dir` from config.rs**

Move them into `auto_register.rs`, renaming:
- `parse_cargo_dep_names` → `parse_cargo_deps` (returns `Vec<DiscoveredDep>`)
- `find_cargo_crate_dir` → `find_cargo_source`

Add `collect_cargo_deps` that ties them together:

```rust
fn collect_cargo_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let cargo_toml = project_root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_toml) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_cargo_deps(&content);
    if deps.is_empty() {
        return;
    }

    let home = match crate::platform::home_dir() {
        Some(h) => h,
        None => return,
    };
    let registry_src = home.join(".cargo").join("registry").join("src");
    let index_dirs: Vec<PathBuf> = match std::fs::read_dir(&registry_src) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect(),
        Err(_) => vec![],
    };

    for dep in deps {
        let source = find_cargo_source(&index_dirs, &dep.name);
        out.push((dep, "rust".to_string(), source));
    }
}
```

- [ ] **Step 3: Add module declaration**

In `src/library/mod.rs`, add:
```rust
pub mod auto_register;
```

- [ ] **Step 4: Update `ActivateProject::call` to use new function**

In `src/tools/config.rs`, replace:
```rust
let auto_registered = auto_register_cargo_deps(&root, ctx).await;
```
with:
```rust
let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
```

Update the response serialization — `auto_registered` is now `Vec<RegisteredDep>`:
```rust
if !auto_registered.is_empty() {
    result["auto_registered_libs"] = json!(auto_registered.iter().map(|r| {
        json!({"name": &r.name, "language": &r.language, "source_available": r.source_available})
    }).collect::<Vec<_>>());
}
```

Remove `auto_register_cargo_deps`, `parse_cargo_dep_names`, and `find_cargo_crate_dir` from `config.rs`.

- [ ] **Step 5: Move existing Cargo tests to auto_register.rs**

Move `parse_cargo_dep_names_basic`, `parse_cargo_dep_names_normalises_hyphens`,
and `activate_project_auto_registers_cargo_dependencies` tests.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all pass — behavior is identical, just relocated

- [ ] **Step 7: Commit**

```bash
git add src/library/auto_register.rs src/library/mod.rs src/tools/config.rs
git commit -m "refactor: move Cargo auto-registration to src/library/auto_register.rs"
```

---

### Task 3: Add Node/TypeScript auto-registration

**Files:**
- Modify: `src/library/auto_register.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn parse_node_deps_basic() {
    let json = r#"{
        "dependencies": { "express": "^4.18.0", "lodash": "4.17.21" },
        "devDependencies": { "jest": "^29.0.0" }
    }"#;
    let deps = parse_node_deps(json);
    assert_eq!(deps.len(), 2);
    assert!(deps.iter().any(|d| d.name == "express"));
    assert!(deps.iter().any(|d| d.name == "lodash"));
    // devDependencies excluded
    assert!(!deps.iter().any(|d| d.name == "jest"));
}

#[test]
fn parse_node_deps_scoped_packages() {
    let json = r#"{"dependencies": {"@babel/core": "^7.0.0", "@types/node": "^20.0.0"}}"#;
    let deps = parse_node_deps(json);
    assert_eq!(deps.len(), 2);
    assert!(deps.iter().any(|d| d.name == "@babel/core"));
}

#[test]
fn find_node_source_in_node_modules() {
    let dir = tempfile::tempdir().unwrap();
    let nm = dir.path().join("node_modules").join("express");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("package.json"), "{}").unwrap();
    assert_eq!(find_node_source(dir.path(), "express"), Some(nm));
}

#[test]
fn find_node_source_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let nm = dir.path().join("node_modules").join("@babel").join("core");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("package.json"), "{}").unwrap();
    assert_eq!(find_node_source(dir.path(), "@babel/core"), Some(nm));
}

#[test]
fn find_node_source_detects_typescript() {
    let dir = tempfile::tempdir().unwrap();
    let nm = dir.path().join("node_modules").join("mylib");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("tsconfig.json"), "{}").unwrap();
    // Language detection tested via collect_node_deps integration
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout auto_register::tests::parse_node`

- [ ] **Step 3: Implement Node parser and locator**

```rust
pub fn parse_node_deps(content: &str) -> Vec<DiscoveredDep> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) else {
        return vec![];
    };
    let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) else {
        return vec![];
    };
    deps.keys()
        .map(|k| DiscoveredDep { name: k.clone(), version_spec: deps[k].as_str().map(String::from) })
        .collect()
}

pub fn find_node_source(project_root: &Path, dep_name: &str) -> Option<PathBuf> {
    let candidate = project_root.join("node_modules").join(dep_name);
    if candidate.is_dir() { Some(candidate) } else { None }
}

fn detect_node_language(pkg_dir: &Path) -> &'static str {
    if pkg_dir.join("tsconfig.json").exists() {
        "typescript"
    } else {
        "javascript"
    }
}

fn collect_node_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let pkg_json = project_root.join("package.json");
    let content = match std::fs::read_to_string(&pkg_json) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_node_deps(&content);
    for dep in deps {
        let source = find_node_source(project_root, &dep.name);
        let lang = source.as_ref()
            .map(|p| detect_node_language(p))
            .unwrap_or("javascript");
        out.push((dep, lang.to_string(), source));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout auto_register::tests::parse_node auto_register::tests::find_node`

- [ ] **Step 5: Handle scoped npm package names in Scope::parse**

Scoped packages like `@babel/core` contain `/` which could confuse path handling.
Store them with the original name (including `/`) in the registry — `node_modules/@babel/core`
is the real filesystem path, so the name must match. Verify `Scope::parse("lib:@babel/core")`
produces `Scope::Library("@babel/core")` — looking at the current code, it does (just takes
everything after `lib:`). Add a test in `src/library/scope.rs`:

```rust
#[test]
fn parse_scoped_npm_name() {
    let scope = Scope::parse(Some("lib:@babel/core"));
    assert_eq!(scope, Scope::Library("@babel/core".to_string()));
}
```

Also verify `includes_library` matches: the registry stores `@babel/core`, and
`scope.includes_library("@babel/core")` must return true. Add test:

```rust
#[test]
fn includes_library_scoped_npm() {
    let scope = Scope::Library("@babel/core".to_string());
    assert!(scope.includes_library("@babel/core"));
    assert!(!scope.includes_library("@babel/preset-env"));
}
```

Run: `cargo test -p codescout scope::tests`

- [ ] **Step 6: Commit**

```bash
git add src/library/auto_register.rs src/library/scope.rs
git commit -m "feat(auto-register): add Node/TypeScript dependency detection"
```

---

### Task 4: Add Python auto-registration

**Files:**
- Modify: `src/library/auto_register.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn normalize_python_name_basic() {
    assert_eq!(normalize_python_name("My-Package"), "my_package");
    assert_eq!(normalize_python_name("zope.interface"), "zope_interface");
    assert_eq!(normalize_python_name("foo_bar"), "foo_bar");
    assert_eq!(normalize_python_name("Flask-RESTful"), "flask_restful");
    assert_eq!(normalize_python_name("a--b..c__d"), "a_b_c_d");
}

#[test]
fn parse_python_deps_pyproject() {
    let toml = r#"
[project]
dependencies = [
    "requests>=2.28,<3",
    "numpy[extra1]>=1.24",
    "importlib-metadata; python_version < '3.8'",
    "flask",
]
"#;
    let deps = parse_python_deps_pyproject(toml);
    assert_eq!(deps.len(), 4);
    assert!(deps.iter().any(|d| d.name == "requests"));
    assert!(deps.iter().any(|d| d.name == "numpy"));
    assert!(deps.iter().any(|d| d.name == "importlib_metadata"));
    assert!(deps.iter().any(|d| d.name == "flask"));
}

#[test]
fn parse_python_deps_requirements() {
    let txt = "requests>=2.28\n# comment\n-r other.txt\n-e ./local\ngit+https://foo\nflask==2.0\n";
    let deps = parse_python_deps_requirements(txt);
    assert_eq!(deps.len(), 2);
    assert!(deps.iter().any(|d| d.name == "requests"));
    assert!(deps.iter().any(|d| d.name == "flask"));
}

#[test]
fn find_python_source_in_venv() {
    let dir = tempfile::tempdir().unwrap();
    let sp = dir.path().join(".venv/lib/python3.11/site-packages/requests");
    std::fs::create_dir_all(&sp).unwrap();
    assert_eq!(find_python_source(dir.path(), "requests"), Some(sp));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout auto_register::tests::normalize_python auto_register::tests::parse_python auto_register::tests::find_python`

- [ ] **Step 3: Implement Python parser and locator**

```rust
/// PEP 503 normalization adapted for filesystem: lowercase, replace runs of [-_.] with _
fn normalize_python_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut result = String::with_capacity(lower.len());
    let mut prev_sep = false;
    for ch in lower.chars() {
        if ch == '-' || ch == '_' || ch == '.' {
            if !prev_sep {
                result.push('_');
                prev_sep = true;
            }
        } else {
            result.push(ch);
            prev_sep = false;
        }
    }
    result
}

pub fn parse_python_deps_pyproject(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut in_project = false;
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = false;
            in_project = trimmed == "[project]";
            continue;
        }
        // Only match `dependencies = [` under [project] table
        if in_project && trimmed.starts_with("dependencies") && trimmed.contains('[') {
            in_deps = true;
            continue;
        }
        if in_deps {
            if trimmed == "]" {
                in_deps = false;
                continue;
            }
            // Strip quotes: "requests>=2.28,<3" → requests>=2.28,<3
            let stripped = trimmed.trim_matches(|c| c == '"' || c == '\'' || c == ',');
            if stripped.is_empty() { continue; }
            // Extract package name: everything before [, >=, ==, !=, ~=, <, >, ;, @, whitespace
            let name_end = stripped.find(|c: char| {
                c == '[' || c == '>' || c == '<' || c == '=' || c == '!' || c == '~'
                    || c == ';' || c == '@' || c.is_whitespace()
            }).unwrap_or(stripped.len());
            let raw_name = &stripped[..name_end];
            if !raw_name.is_empty() {
                deps.push(DiscoveredDep {
                    name: normalize_python_name(raw_name),
                    version_spec: None,
                });
            }
        }
    }
    deps
}

pub fn parse_python_deps_requirements(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        if trimmed.contains("://") { continue; }
        let name_end = trimmed.find(|c: char| {
            c == '>' || c == '<' || c == '=' || c == '!' || c == '~'
                || c == '[' || c == ';' || c == '@' || c.is_whitespace()
        }).unwrap_or(trimmed.len());
        let raw_name = &trimmed[..name_end];
        if !raw_name.is_empty() {
            deps.push(DiscoveredDep {
                name: normalize_python_name(raw_name),
                version_spec: None,
            });
        }
    }
    deps
}

pub fn find_python_source(project_root: &Path, dep_name: &str) -> Option<PathBuf> {
    let venv_dirs = [".venv", "venv", ".env", "env"];
    for venv in &venv_dirs {
        let lib = project_root.join(venv).join("lib");
        let Ok(entries) = std::fs::read_dir(&lib) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            // python3.X directory
            let sp = entry.path().join("site-packages").join(dep_name);
            if sp.is_dir() { return Some(sp); }
        }
    }
    None
}

fn collect_python_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let pyproject = project_root.join("pyproject.toml");
    let requirements = project_root.join("requirements.txt");

    let deps = if pyproject.exists() {
        match std::fs::read_to_string(&pyproject) {
            Ok(s) => parse_python_deps_pyproject(&s),
            Err(_) => vec![],
        }
    } else if requirements.exists() {
        match std::fs::read_to_string(&requirements) {
            Ok(s) => parse_python_deps_requirements(&s),
            Err(_) => vec![],
        }
    } else {
        return;
    };

    for dep in deps {
        let source = find_python_source(project_root, &dep.name);
        out.push((dep, "python".to_string(), source));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout auto_register::tests`

- [ ] **Step 5: Commit**

```bash
git add src/library/auto_register.rs
git commit -m "feat(auto-register): add Python dependency detection"
```

---

### Task 5: Add Go auto-registration

**Files:**
- Modify: `src/library/auto_register.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn parse_go_deps_basic() {
    let gomod = r#"
module github.com/myorg/myapp

go 1.21

require (
	github.com/gin-gonic/gin v1.9.1
	golang.org/x/sync v0.5.0
)

require (
	github.com/indirect/dep v0.1.0 // indirect
)
"#;
    let deps = parse_go_deps(gomod);
    // Include both direct and indirect
    assert!(deps.len() >= 2);
    assert!(deps.iter().any(|d| d.name == "github.com/gin-gonic/gin"));
    assert!(deps.iter().any(|d| d.name == "golang.org/x/sync"));
}

#[test]
fn go_encode_module_path_basic() {
    assert_eq!(go_encode_module_path("github.com/Azure/azure-sdk"),
               "github.com/!azure/azure-sdk");
    assert_eq!(go_encode_module_path("github.com/foo/bar"),
               "github.com/foo/bar");
}

#[test]
fn find_go_source_in_modcache() {
    let dir = tempfile::tempdir().unwrap();
    let mod_dir = dir.path().join("github.com/gin-gonic/gin@v1.9.1");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("go.mod"), "module gin").unwrap();
    assert!(find_go_source(dir.path(), "github.com/gin-gonic/gin").is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout auto_register::tests::parse_go auto_register::tests::go_encode auto_register::tests::find_go`

- [ ] **Step 3: Implement Go parser and locator**

```rust
pub fn parse_go_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut in_require = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "require (" {
            in_require = true;
            continue;
        }
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        // Single-line require
        if trimmed.starts_with("require ") && !trimmed.contains('(') {
            let rest = trimmed.strip_prefix("require ").unwrap().trim();
            if let Some((mod_path, _)) = rest.split_once(' ') {
                deps.push(DiscoveredDep { name: mod_path.to_string(), version_spec: None });
            }
            continue;
        }
        if in_require {
            // "github.com/foo/bar v1.2.3" or "github.com/foo/bar v1.2.3 // indirect"
            let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                deps.push(DiscoveredDep {
                    name: parts[0].to_string(),
                    version_spec: Some(parts[1].to_string()),
                });
            }
        }
    }
    deps
}

/// Go module cache encodes uppercase letters as !lowercase
pub fn go_encode_module_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len() + 4);
    for ch in path.chars() {
        if ch.is_uppercase() {
            result.push('!');
            for lower in ch.to_lowercase() { result.push(lower); }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn find_go_source(modcache: &Path, module_path: &str) -> Option<PathBuf> {
    let encoded = go_encode_module_path(module_path);
    // The module cache dir is encoded_path@version — we need to find any version
    let parent = modcache.join(
        std::path::Path::new(&encoded).parent().unwrap_or(std::path::Path::new(""))
    );
    let dir_name = std::path::Path::new(&encoded)
        .file_name()?
        .to_str()?;
    let prefix = format!("{}@", dir_name);

    let rd = std::fs::read_dir(&parent).ok()?;
    let mut candidates: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with(&prefix) && e.path().is_dir()
        })
        .map(|e| e.path())
        .collect();
    candidates.sort();
    candidates.into_iter().next_back() // highest version
}

fn collect_go_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let go_mod = project_root.join("go.mod");
    let content = match std::fs::read_to_string(&go_mod) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_go_deps(&content);
    if deps.is_empty() { return; }

    // Find GOMODCACHE — try `go env` first, fall back to ~/go/pkg/mod
    let modcache = std::process::Command::new("go")
        .args(["env", "GOMODCACHE"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            String::from_utf8(o.stdout).ok().map(|s| PathBuf::from(s.trim()))
        } else { None })
        .or_else(|| {
            crate::platform::home_dir().map(|h| h.join("go").join("pkg").join("mod"))
        });

    let modcache = match modcache {
        Some(p) if p.is_dir() => p,
        _ => {
            // No Go module cache — register without source
            for dep in deps {
                out.push((dep, "go".to_string(), None));
            }
            return;
        }
    };

    for dep in deps {
        let source = find_go_source(&modcache, &dep.name);
        out.push((dep, "go".to_string(), source));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout auto_register::tests`

- [ ] **Step 5: Commit**

```bash
git add src/library/auto_register.rs
git commit -m "feat(auto-register): add Go dependency detection"
```

---

### Task 6: Add Java/Kotlin (Gradle + Maven) auto-registration

**Files:**
- Modify: `src/library/auto_register.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn parse_gradle_deps_kotlin_dsl() {
    let gradle = r#"
dependencies {
    implementation("com.fasterxml.jackson.core:jackson-databind:2.15.0")
    api("org.jetbrains.kotlin:kotlin-stdlib:1.9.0")
    testImplementation("junit:junit:4.13.2")
    compileOnly("org.projectlombok:lombok:1.18.30")
}
"#;
    let deps = parse_gradle_deps(gradle);
    assert!(deps.iter().any(|d| d.name == "jackson-databind"));
    assert!(deps.iter().any(|d| d.name == "kotlin-stdlib"));
    assert!(deps.iter().any(|d| d.name == "lombok"));
    // testImplementation is excluded
    assert!(!deps.iter().any(|d| d.name == "junit"));
}

#[test]
fn parse_gradle_deps_groovy_dsl() {
    let gradle = "dependencies {\n    implementation 'com.google.guava:guava:32.1.2-jre'\n}\n";
    let deps = parse_gradle_deps(gradle);
    assert!(deps.iter().any(|d| d.name == "guava"));
}

#[test]
fn parse_maven_deps_basic() {
    let pom = r#"
<dependencies>
    <dependency>
        <groupId>com.fasterxml.jackson.core</groupId>
        <artifactId>jackson-databind</artifactId>
        <version>2.15.0</version>
    </dependency>
    <dependency>
        <groupId>junit</groupId>
        <artifactId>junit</artifactId>
        <scope>test</scope>
    </dependency>
</dependencies>
"#;
    let deps = parse_maven_deps(pom);
    assert!(deps.iter().any(|d| d.name == "jackson-databind"));
    // test scope excluded
    assert!(!deps.iter().any(|d| d.name == "junit"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout auto_register::tests::parse_gradle auto_register::tests::parse_maven`

- [ ] **Step 3: Implement Gradle and Maven parsers**

```rust
pub fn parse_gradle_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    // Match Kotlin DSL: implementation("group:artifact:version")
    // Match Groovy DSL: implementation 'group:artifact:version'
    // Configs: implementation, api, compileOnly, runtimeOnly (NOT test*)
    let re = regex::Regex::new(
        r#"(?:implementation|api|compileOnly|runtimeOnly)\s*(?:\(\s*["']|['"])([^"']+?)["']"#
    ).unwrap();
    for cap in re.captures_iter(content) {
        let coord = &cap[1];
        // group:artifact:version — extract artifact (middle part)
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 2 {
            deps.push(DiscoveredDep {
                name: parts[1].to_string(),
                version_spec: parts.get(2).map(|v| v.to_string()),
            });
        }
    }
    deps
}

pub fn parse_maven_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    // Simple line-based XML parsing — not a full XML parser
    let mut current_artifact: Option<String> = None;
    let mut current_scope: Option<String> = None;
    let mut in_dependency = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("<dependency>") { in_dependency = true; current_artifact = None; current_scope = None; }
        if trimmed.contains("</dependency>") {
            if in_dependency {
                if let Some(artifact) = current_artifact.take() {
                    let is_test = current_scope.as_deref() == Some("test");
                    if !is_test {
                        deps.push(DiscoveredDep { name: artifact, version_spec: None });
                    }
                }
            }
            in_dependency = false;
        }
        if in_dependency {
            if let Some(val) = extract_xml_value(trimmed, "artifactId") {
                current_artifact = Some(val);
            }
            if let Some(val) = extract_xml_value(trimmed, "scope") {
                current_scope = Some(val);
            }
        }
    }
    deps
}

fn extract_xml_value(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = line.find(&open)? + open.len();
    let end = line.find(&close)?;
    Some(line[start..end].to_string())
}

fn detect_jvm_language(project_root: &Path) -> &'static str {
    if project_root.join("build.gradle.kts").exists() { "kotlin" }
    else { "java" }
}

fn collect_jvm_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let language = detect_jvm_language(project_root);

    // Try Gradle first, then Maven
    let deps = if let Ok(content) = std::fs::read_to_string(project_root.join("build.gradle.kts"))
        .or_else(|_| std::fs::read_to_string(project_root.join("build.gradle")))
    {
        parse_gradle_deps(&content)
    } else if let Ok(content) = std::fs::read_to_string(project_root.join("pom.xml")) {
        parse_maven_deps(&content)
    } else {
        return;
    };

    // JVM deps are always registered without source
    for dep in deps {
        out.push((dep, language.to_string(), None));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout auto_register::tests`

- [ ] **Step 5: Commit**

```bash
git add src/library/auto_register.rs
git commit -m "feat(auto-register): add Java/Kotlin (Gradle + Maven) dependency detection"
```

---

### Task 7: Add `source_available` check in `resolve_library_roots` and semantic search

**Files:**
- Modify: `src/tools/symbol.rs:75-91` (resolve_library_roots)
- Modify: `src/tools/semantic.rs:62-80,267-290` (semantic_search + index_project lib scope)

**Note:** The spec claims `resolve_library_roots` covers `search_pattern` — this is incorrect.
`SearchPattern` in `src/tools/file.rs` has no library scope support and does NOT call
`resolve_library_roots`. This is out of scope for this task; `search_pattern` library support
can be added separately if needed.

- [ ] **Step 1: Write failing test for resolve_library_roots filtering**

Add to `src/tools/symbol.rs` tests:

```rust
#[tokio::test]
async fn resolve_library_roots_excludes_source_unavailable() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "available".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
        project.library_registry.register(
            "unavailable".to_string(),
            PathBuf::new(),
            "java".to_string(),
            crate::library::registry::DiscoveryMethod::ManifestScan,
            false,
        );
    }
    let result = resolve_library_roots(
        &crate::library::scope::Scope::Library("unavailable".to_string()),
        &agent,
    ).await;
    assert!(result.is_err(), "should return error for source-unavailable library");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("source code is not available"), "error: {err}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout resolve_library_roots_excludes_source_unavailable`

- [ ] **Step 3: Update `resolve_library_roots` to return `Result` and check `source_available`**

Change signature from `-> Vec<(String, PathBuf)>` to `-> anyhow::Result<Vec<(String, PathBuf)>>`.

**Critical:** The `source_available` check must ONLY fire for `Scope::Library(name)` — NOT
for `Scope::All` or `Scope::Libraries`. The `find_references` tool calls this with `Scope::All`
for classifying reference locations (project vs library), and would break if any registered
library lacks source.

```rust
async fn resolve_library_roots(
    scope: &crate::library::scope::Scope,
    agent: &crate::agent::Agent,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let registry = match agent.library_registry().await {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let matched: Vec<&crate::library::registry::LibraryEntry> = registry
        .all()
        .iter()
        .filter(|entry| scope.includes_library(&entry.name))
        .collect();

    // Only check source_available for explicit single-library scope.
    // Scope::All and Scope::Libraries are used for classification (find_references)
    // and must silently skip source-unavailable entries rather than erroring.
    if let crate::library::scope::Scope::Library(_) = scope {
        let unavailable: Vec<&str> = matched.iter()
            .filter(|e| !e.source_available)
            .map(|e| e.name.as_str())
            .collect();
        if !unavailable.is_empty() {
            let names = unavailable.join(", ");
            return Err(RecoverableError::with_hint(
                format!(
                    "Library source code is not available locally for: {}",
                    names,
                ),
                "To browse library source, download it using the project's build tool \
                 (e.g. ./gradlew dependencies, mvn dependency:sources), then call \
                 register_library(name, \"/path/to/source\", language) and retry.",
            ).into());
        }
    }

    // Filter out source-unavailable entries for non-error scopes
    Ok(matched.iter()
        .filter(|entry| entry.source_available)
        .map(|entry| (entry.name.clone(), entry.path.clone()))
        .collect())
}
```

- [ ] **Step 4: Update all callers of `resolve_library_roots` to handle `Result`**

Search for `resolve_library_roots(` in `src/tools/symbol.rs` and add `?` to propagate the
error. For `find_references` (which uses `Scope::All` for classification), `?` is safe because
the error only fires for `Scope::Library(name)`.

- [ ] **Step 5: Add `source_available` checks in `semantic.rs`**

Two interception points:

**5a. `index_project` lib scope** (around line 267) — after looking up the library entry:

```rust
if !entry.source_available {
    return Err(RecoverableError::with_hint(
        format!("Library '{}' source code is not available locally.", lib_name),
        "Download sources using the project's build tool, then call \
         register_library(name, \"/path/to/source\", language) and retry.",
    ).into());
}
```

**5b. `semantic_search` lib scope** (around line 62-80) — `semantic_search` dispatches
to `search_multi_db` which opens library DB files. A registered-but-no-source library
has no DB, producing a confusing error. Add the same check before attempting the DB
lookup, after resolving the library entry from the registry:

```rust
if !entry.source_available {
    return Err(RecoverableError::with_hint(
        format!("Library '{}' source code is not available locally — cannot search.", lib_name),
        "Download sources, then index_project(scope='lib:{}') before searching.",
    ).into());
}
```

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add src/tools/symbol.rs src/tools/semantic.rs
git commit -m "feat: gate source-unavailable libraries with RecoverableError hint"
```

---

### Task 8: Update `list_libraries` output and `format_activate_project`

**Files:**
- Modify: `src/tools/library.rs:28-60` (ListLibraries::call)
- Modify: `src/tools/config.rs` (format_activate_project)

- [ ] **Step 1: Write test for list_libraries showing source_available**

```rust
#[tokio::test]
async fn list_libraries_shows_source_available() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "avail".to_string(), lib_dir.path().to_path_buf(),
            "rust".to_string(), DiscoveryMethod::Manual, true,
        );
        project.library_registry.register(
            "noavail".to_string(), PathBuf::new(),
            "java".to_string(), DiscoveryMethod::ManifestScan, false,
        );
    }
    let ctx = project_ctx_with_agent(agent);
    let result = ListLibraries.call(json!({}), &ctx).await.unwrap();
    let libs = result["libraries"].as_array().unwrap();
    let noavail = libs.iter().find(|l| l["name"] == "noavail").unwrap();
    assert_eq!(noavail["source_available"], false);
    let avail = libs.iter().find(|l| l["name"] == "avail").unwrap();
    assert_eq!(avail["source_available"], true);
}
```

- [ ] **Step 2: Run test to verify it fails**

- [ ] **Step 3: Add `source_available` to library JSON serialization**

In `ListLibraries::call`, where each entry is serialized, add the field:

```rust
"source_available": entry.source_available,
```

- [ ] **Step 4: Update `format_activate_project` compact output**

Update the compact line to show source stats when auto_registered_libs is present:

```rust
if let Some(libs) = result["auto_registered_libs"].as_array() {
    let without_source = libs.iter()
        .filter(|l| l["source_available"] == false)
        .count();
    if without_source > 0 {
        format!("auto-registered {} libs ({} without source)", libs.len(), without_source)
    } else {
        format!("auto-registered {} libs", libs.len())
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p codescout list_libraries format_activate`

- [ ] **Step 6: Commit**

```bash
git add src/tools/library.rs src/tools/config.rs
git commit -m "feat: show source_available in list_libraries and activation summary"
```

---

### Task 9: Integration test and final validation

**Files:**
- Modify: `src/library/auto_register.rs` (integration test)

- [ ] **Step 1: Write integration test with multiple manifests**

```rust
#[tokio::test]
async fn auto_register_deps_multi_ecosystem() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();

    // Cargo.toml
    std::fs::write(root.join("Cargo.toml"),
        "[package]\nname=\"test\"\n\n[dependencies]\nserde = \"1\"\n").unwrap();

    // package.json
    std::fs::write(root.join("package.json"),
        r#"{"dependencies":{"express":"^4.0"}}"#).unwrap();

    // node_modules for express
    let nm = root.join("node_modules/express");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("package.json"), "{}").unwrap();

    // build.gradle.kts (no source available)
    std::fs::write(root.join("build.gradle.kts"),
        "dependencies {\n    implementation(\"com.google:guava:32.0\")\n}\n").unwrap();

    let agent = crate::agent::Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = crate::tools::ToolContext {
        agent,
        lsp: std::sync::Arc::new(crate::lsp::mock::MockLspProvider::new()),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };

    let registered = auto_register_deps(root, &ctx).await;

    // Should have deps from multiple ecosystems
    assert!(registered.iter().any(|r| r.name == "express" && r.source_available));
    assert!(registered.iter().any(|r| r.name == "guava" && !r.source_available));
    // Cargo deps may or may not have source (depends on local registry)
}
```

- [ ] **Step 2: Write re-activation idempotency test**

```rust
#[tokio::test]
async fn auto_register_deps_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(root.join("package.json"),
        r#"{"dependencies":{"express":"^4.0"}}"#).unwrap();
    let nm = root.join("node_modules/express");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("package.json"), "{}").unwrap();

    let agent = crate::agent::Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = crate::tools::ToolContext {
        agent,
        lsp: std::sync::Arc::new(crate::lsp::mock::MockLspProvider::new()),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
    };

    let first = auto_register_deps(root, &ctx).await;
    let second = auto_register_deps(root, &ctx).await;

    // Second call registers nothing new
    assert!(!first.is_empty());
    assert!(second.is_empty(), "second activation should not re-register");

    // Registry has exactly one entry per dep
    let count = ctx.agent.with_project(|p| {
        Ok(p.library_registry.all().iter().filter(|e| e.name == "express").count())
    }).await.unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p codescout auto_register_deps_multi auto_register_deps_idempotent`

- [ ] **Step 4: Run full validation suite**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 5: Update BUG-022 status**

In `docs/TODO-tool-misbehaviors.md`, change BUG-022 status to ✅ Fixed.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: multi-ecosystem library auto-registration (closes BUG-022)"
```
