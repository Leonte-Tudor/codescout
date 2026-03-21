# Multi-Ecosystem Library Auto-Registration

**Date:** 2026-03-21
**Status:** Draft
**Branch:** experiments

## Problem

BUG-022 identified that agents bypass codescout's structured library navigation and
fall back to raw `grep` on package caches when dependencies aren't registered.
Currently, only Rust/Cargo dependencies are auto-registered during `activate_project`.
Projects using Node, Python, Go, or Java/Kotlin get no auto-registration, forcing agents
to discover libraries manually or not at all.

## Goal

Auto-register dependencies from all supported ecosystems at activation time, so agents
see the full dependency graph in `list_libraries` and can use `scope="lib:X"` for
structured navigation. For ecosystems where source code isn't locally available
(Java/Kotlin), provide a clear signal that lets the agent download sources on demand.

## Design

### 1. New Module: `src/library/auto_register.rs`

Extracts all auto-registration logic from `src/tools/config.rs` into a dedicated module.

**Entry point:**

```rust
pub async fn auto_register_deps(
    project_root: &Path,
    ctx: &ToolContext,
) -> Vec<RegisteredDep>
```

Detects which manifests exist in the project root and dispatches to per-ecosystem
handlers. Returns a list of what was newly registered.

```rust
pub struct RegisteredDep {
    pub name: String,
    pub language: String,
    pub source_available: bool,
}
```

**Per-ecosystem handlers:**

Each ecosystem has two functions:
- `parse_<ecosystem>_deps(content: &str) -> Vec<DiscoveredDep>` — extract dep names from manifest
- `find_<ecosystem>_source(project_root: &Path, dep_name: &str) -> Option<PathBuf>` — locate source on disk

```rust
pub struct DiscoveredDep {
    pub name: String,
    pub version_spec: Option<String>,  // for future version tracking
}
```

### 2. Ecosystem Details

#### Rust (existing — moved from config.rs)

- **Manifest:** `Cargo.toml`
- **Parse:** `[dependencies]` section; skip `[dev-dependencies]`, `[build-dependencies]`
- **Source:** `~/.cargo/registry/src/index.crates.io-*/<name>-<version>/`
- **Name normalization:** `-` ↔ `_` (Cargo treats as equivalent)
- **Language:** `"rust"`

#### Node/TypeScript

- **Manifest:** `package.json`
- **Parse:** `dependencies` object keys; skip `devDependencies`
- **Source:** `<project_root>/node_modules/<name>/` (scoped: `node_modules/@scope/name/`)
- **Language:** Check for `tsconfig.json` in package dir → `"typescript"`, else `"javascript"`
- **Scoped packages:** `@scope/name` is the registry name. For `Scope::parse`, normalize
  `/` to `__` in the library name (e.g., `lib:@babel__core`). The original scoped name is
  stored in `LibraryEntry` metadata for display, while the normalized name is used for
  `scope=` lookups. Verify `Scope::parse` handles this correctly.

#### Python

- **Manifest:** `pyproject.toml` (preferred) or `requirements.txt` (fallback)
- **Parse:**
  - `pyproject.toml`: `[project.dependencies]` array entries, strip version specifiers
    (`>=`, `==`, `~=`, etc.), extras (`[extra1,extra2]`), and environment markers
    (`; python_version < "3.8"`)
  - `requirements.txt`: one dep per line, same stripping. Skip lines starting with
    `-` (pip options like `-r`, `-e`), `#` (comments), or containing `://` (VCS URLs).
- **Source:** Scan for `.venv/lib/python*/site-packages/<name>/` in project root.
  Also check `venv/`, `.env/`, `env/`. Do NOT scan system site-packages.
- **Name normalization:** `normalize_python_name` helper — lowercase, replace any run
  of `[-_.]` with `_` (PEP 503 normalization adapted for filesystem lookup). Test cases:
  `zope.interface` → `zope_interface`, `My-Package` → `my_package`, `foo_bar` → `foo_bar`.
- **Language:** `"python"`

#### Go

- **Manifest:** `go.mod`
- **Parse:** `require (...)` block; extract module paths
- **Source:** `$GOMODCACHE` (default `~/go/pkg/mod/`). Dirs use Go's case-encoding
  (`!` prefix for uppercase), e.g. `github.com/!azure/azure-sdk-for-go@v1.2.3/`.
  Requires a `go_encode_module_path` helper to convert module paths to filesystem paths.
- **Discovery:** Shell out to `go env GOMODCACHE` once during activation to find cache dir.
  If `go` is not on PATH, skip Go deps silently (consistent with never-fail pattern).
- **Language:** `"go"`

#### Java/Kotlin (Gradle / Maven)

- **Manifest:** `build.gradle.kts`, `build.gradle`, or `pom.xml`
- **Parse:**
  - Gradle: `implementation("group:artifact:version")` and `api("...")`,
    `compileOnly("...")`, `runtimeOnly("...")` patterns. Groovy DSL single-quote
    variant: `implementation 'group:artifact:version'`. Variable interpolation and
    version catalogs (`libs.foo.bar`) are NOT supported — known v1 limitation.
  - Maven: `<dependency><groupId>...<artifactId>...` blocks
- **Source:** Always `None` — local caches contain compiled bytecode (JARs), not source.
  Source JARs exist but are not downloaded by default.
- **Language:** `.gradle.kts` → `"kotlin"`, `.gradle` / `pom.xml` → `"java"`
- **Registered with `source_available: false`** — see Section 4 for the download flow.

### 3. Registry Changes

**3a. New `DiscoveryMethod` variant:**

```rust
pub enum DiscoveryMethod {
    Manual,           // user called register_library
    LspFollowThrough, // goto_definition auto-discovery
    ManifestScan,     // NEW — auto-registered from manifest during activation
}
```

`ManifestScan` entries can be safely overwritten on re-activation. `Manual` entries
are preserved — if a user explicitly registered a library, activation must not
overwrite it with a potentially different path. The `register()` method skips
updates when the existing entry has `discovered_via: Manual` and the new call
uses `ManifestScan`.

**3b. `source_available` field and `path` semantics:**

```rust
pub struct LibraryEntry {
    pub name: String,
    pub version: Option<String>,
    pub version_indexed: Option<String>,
    pub db_file: Option<String>,
    pub nudge_dismissed: bool,
    pub path: PathBuf,
    pub language: String,
    pub indexed: bool,
    pub discovered_via: DiscoveryMethod,
    #[serde(default = "default_true")]
    pub source_available: bool,  // NEW
}
```

- `source_available: true` (default): source code is on disk at `path`
- `source_available: false`: registered from manifest, source not locally available.
  `path` is set to `PathBuf::new()` (empty). Tools must check `source_available`
  before attempting to read from `path`.

Default is `true` for backward compatibility — existing `libraries.json` entries
without this field are assumed to have source available.

**3c. `register()` signature update:**

```rust
pub fn register(
    &mut self,
    name: String,
    path: PathBuf,
    language: String,
    discovered_via: DiscoveryMethod,
    source_available: bool,  // NEW
)
```

When `discovered_via` is `ManifestScan` and an existing entry has `Manual`,
the update is skipped (user registration takes precedence).

**3d. `list_libraries` output** includes `source_available` so agents see which
libraries are ready for navigation vs. which need source downloads.

### 4. Source-Not-Available Flow (RecoverableError)

**Primary interception point:** `resolve_library_roots()` in `src/tools/symbol.rs`
(line ~77). This is the single dispatch point for all scope-based library resolution —
it covers `find_symbol`, `list_symbols`, `find_references`, and `search_pattern`.
After filtering by `scope.includes_library(&entry.name)`, add a `source_available`
check. If any matched library has `source_available: false`, return a `RecoverableError`.

**Secondary interception point:** `semantic_search` scope resolution in
`src/tools/semantic.rs` (line ~267) and `index_project` scope resolution, which
handle `scope="lib:X"` independently. Same check needed.

When a tool encounters `scope="lib:X"` with `source_available: false`:

The tool returns a `RecoverableError`:

```
Library 'jackson-core' is registered but source code is not available locally.

To browse this library's source code, download it using the project's build tool:
  - Gradle: ./gradlew dependencies (or a sources-download task)
  - Maven: mvn dependency:sources

After downloading, call register_library("jackson-core", "/path/to/source", "java")
to make it available, then retry your query.

To skip, omit the scope parameter to search project code only.
```

**Key design choice:** codescout does NOT build download commands into itself.
The agent already knows the build tool and has `run_command`. We provide the
signal ("source not available") and let the agent handle the download. This
keeps codescout build-tool-agnostic.

The flow is naturally user-consented: the agent describes what it's about to do,
and the user can approve/deny the `run_command` via Claude Code's permission system.

### 5. Integration Point

In `src/tools/config.rs`, `ActivateProject::call`:

```rust
// Replace:
let auto_registered = auto_register_cargo_deps(&root, ctx).await;

// With:
let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
```

The `auto_register_cargo_deps`, `parse_cargo_dep_names`, and `find_cargo_crate_dir`
functions move to `src/library/auto_register.rs`. The existing tests move with them.

**Batching strategy:** `auto_register_deps` collects all deps from all ecosystems
first, then acquires the write lock once and registers them in a single pass.
This is simpler and faster than the current per-dep lock/save pattern. The registry
is saved once at the end.

**Activation response:** The `auto_registered_libs` field in the activation response
changes from `Vec<String>` (just names) to include language and source status:

```json
"auto_registered_libs": [
  {"name": "tokio", "language": "rust", "source_available": true},
  {"name": "jackson-core", "language": "java", "source_available": false}
]
```

The `format_activate_project` compact formatter shows a summary like:
`"auto-registered 12 libs (3 without source)"`.

### 6. What This Does NOT Include

- **No `search_pattern` hints for cache paths** — auto-registration makes this
  unnecessary; agents see deps in `list_libraries` and use `scope="lib:..."`.
- **No automatic source downloading** — the agent handles downloads using
  `run_command` with the project's build tool.
- **No system-wide package scanning** — Python only checks project-local venvs,
  not system site-packages.
- **BUG-022 closure** — this design fully addresses BUG-022 by making libraries
  discoverable across all ecosystems.

## Testing Strategy

**Parser tests (per ecosystem):**
- Each `parse_<ecosystem>_deps` function gets tests with realistic manifest content,
  edge cases (scoped packages, version specifiers, comments, empty sections).
- Python: `normalize_python_name` tested with `zope.interface`, `My-Package`, `foo_bar`.
- Python `requirements.txt`: skip `-r`, `-e`, `#`, `://` lines.
- Go: `go_encode_module_path` tested with uppercase module paths.
- Gradle: both Kotlin DSL (`("...")`) and Groovy DSL (`'...'`) patterns.
- Node: scoped packages (`@scope/name`) correctly parsed and normalized.

**Locator tests (per ecosystem):**
- Each `find_<ecosystem>_source` function gets tests using tempdir fixtures with
  expected directory structures.
- Go: fallback when `go` binary not on PATH (returns `None`, no error).

**Registry tests:**
- `source_available` field: backward compat — `libraries.json` without the field
  deserializes with default `true`.
- `register()` with `ManifestScan` does NOT overwrite existing `Manual` entry.
- `register()` with `ManifestScan` DOES update existing `ManifestScan` entry.

**Integration tests:**
- `activate_project` on a fixture with multiple manifests registers deps from
  all detected ecosystems.
- Re-activation idempotency: running `activate_project` twice does not duplicate
  entries or lose user-registered libraries.
- `list_libraries` output includes `source_available` field.

**RecoverableError tests:**
- `resolve_library_roots` filters out `source_available: false` entries and returns
  `RecoverableError` with download hint.
- `find_symbol(scope="lib:X")` with `source_available: false` returns the error.
- Scoped npm names round-trip through `Scope::parse` correctly.

## File Changes

| File | Change |
|------|--------|
| `src/library/mod.rs` | Add `pub mod auto_register;` |
| `src/library/auto_register.rs` | **New** — all parsing/locating/registration logic |
| `src/library/registry.rs` | Add `source_available` field, `DiscoveryMethod::ManifestScan`, update `register()` |
| `src/library/scope.rs` | Verify/fix `Scope::parse` for scoped npm names with `/` |
| `src/tools/config.rs` | Replace `auto_register_cargo_deps` call; remove moved functions; update activation response |
| `src/tools/symbol.rs` | Add `source_available` check in `resolve_library_roots()` |
| `src/tools/semantic.rs` | Add `source_available` check in scope resolution |
| `src/tools/library.rs` | `list_libraries` serializes `source_available`; `register_library` accepts new field |
| `docs/TODO-tool-misbehaviors.md` | Update BUG-022 status to ✅ Fixed |

## Known v1 Limitations

- Gradle version catalogs (`libs.foo.bar`) and variable interpolation not parsed
- Go module cache requires `go` binary on PATH; skipped otherwise
- Python only scans project-local venvs, not conda/system/pipx environments
- No automatic source downloading — agent handles via `run_command`
