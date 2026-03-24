# Library Indexing Redesign

**Date:** 2026-03-13
**Branch:** experiments
**Status:** Design approved, pending implementation plan

## Problem Statement

Library indexing in codescout is underutilized because:

1. **Agent UX gap (primary):** Agents don't naturally know when or which library to index. Auto-discovery via `tag_external_path` is dead code. The `indexed: bool` flag only helps if the agent proactively calls `list_libraries`.
2. **No version tracking:** `LibraryEntry.version` exists but is usually `None`. Dependency updates silently leave stale embeddings.
3. **No clean separation:** Project and library chunks share one `embeddings.db`, making lifecycle management (drop one library, migrate, etc.) messy.
4. **Read-only navigation blocked:** Tools like `list_symbols`, `find_symbol`, `read_file` reject library paths because they're outside the project root, even though the source is available on disk.

## Design Decisions

- **Separate SQLite DB per library** — not a different engine, just clean file separation
- **Primary focus:** single developer, single large project with 5-10 dependencies
- **Nudge-then-act UX:** hint on first library contact, optional auto-index via config
- **Version-aware staleness:** detect version changes from lockfiles, suggest re-index
- **JVM source fetching:** auto-download sources via `mvn`/`gradle` when not on disk
- **Full read-only navigation** for registered libraries (not just semantic search)

## Assumptions

- Libraries have source code on disk for all ecosystems except JVM (where sources can be fetched)
- The embedding model and chunk size are the same for libraries as for project code
- Library code changes infrequently — version changes are the primary staleness signal
- A typical project has 5-10 indexed libraries; the design does not optimize for 50+
- Library DBs are small enough (< 50 MB each) that opening/closing per search is acceptable for v1

## Section 1: Directory Structure & DB Separation

### Storage layout

```
.codescout/
  embeddings/
    project.db              # project-only chunks (migrated from embeddings.db)
    lib/
      tokio.db              # one DB per library
      serde.db
      spring-boot.db
  libraries.json            # registry with version tracking
```

### Library DB filename convention

Library names are sanitized for filesystem safety:
- Forward slashes and backslashes → `--` (e.g., `@scope/name` → `@scope--name.db`)
- Names are lowercased on case-insensitive filesystems (macOS, Windows)
- If two libraries produce the same sanitized filename (collision), append a 4-char hash of the original path: `utils-a3f2.db`
- The canonical name-to-filename mapping is stored in `libraries.json` (`db_file` field) so there is no ambiguity

### Schema per library DB

Identical table structure to project DB (`code_chunks`, `chunk_embeddings`, `file_hashes`), plus a metadata table:

```sql
CREATE TABLE IF NOT EXISTS lib_meta (
    key TEXT PRIMARY KEY,
    value TEXT
);
-- Stores: version_indexed, language, indexed_at, source_path
```

### `libraries.json` additions

```json
{
  "name": "tokio",
  "version": "1.38.0",
  "version_indexed": "1.38.0",
  "db_file": "tokio.db",
  "path": "/home/user/.cargo/registry/src/.../tokio-1.38.0",
  "language": "rust",
  "indexed": true,
  "discovered_via": "LspFollowThrough",
  "nudge_dismissed": false
}
```

When `version != version_indexed`, the library is stale. The `nudge_dismissed` field (default `false`) persists across sessions — if the user has seen a nudge and chosen not to index, they won't be re-nudged until the version changes.

## Section 2: Multi-Language Version Detection

### Lockfile resolution by project type

| Project type | Detection file | Lockfile(s) to parse | Version extraction | Priority |
|---|---|---|---|---|
| Rust | `Cargo.toml` | `Cargo.lock` | `[[package]]` entries: name + version | **P0 — v1** |
| JS/TS | `package.json` | `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml` (try in order) | Resolved version per package | **P0 — v1** |
| Go | `go.mod` | `go.sum` | Module path includes version | **P1 — v1** |
| Python | `pyproject.toml` | `uv.lock`, `poetry.lock`, `requirements.txt` (try in order) | Pinned versions | **P1 — v1** |
| Java (Maven) | `pom.xml` | `pom.xml` resolved deps | `<dependency>` version tags | **P2 — v1 best-effort** |
| Kotlin/Java (Gradle) | `build.gradle.kts` / `build.gradle` | `gradle.lockfile` if present | Dependency declarations | **P2 — deferred without lockfile** |

**Implementation order:** Rust and JS/TS first (P0), then Go and Python (P1), then JVM (P2). Gradle-without-lockfile (parsing Groovy/Kotlin DSL) is deferred — too complex for v1.

**Manual override:** For ecosystems where lockfile parsing is not yet implemented, users can set versions manually:

```toml
[libraries.version_overrides]
spring-boot = "3.2.1"
```

### New module: `src/library/versions.rs`

```rust
pub struct ResolvedVersion {
    pub name: String,
    pub version: String,
}

/// Given a project root, detect the project type and parse
/// the lockfile to get resolved dependency versions.
pub fn resolve_dependency_versions(project_root: &Path) -> Vec<ResolvedVersion>
```

### When version checking runs

- On `semantic_search` — compare `version` vs `version_indexed`. If stale, include hint:
  ```json
  {
    "stale_libraries": [
      { "name": "tokio", "indexed": "1.37.0", "current": "1.38.0",
        "hint": "tokio was updated — run index_project(scope='lib:tokio') to re-index" }
    ]
  }
  ```
- On `list_libraries` — show version comparison inline
- **Not** on every tool call — only search-related tools where stale data matters

### Version refresh

When `index_project(scope="lib:<name>")` completes, it reads the current version from the lockfile and writes it to both `libraries.json` (`version` + `version_indexed`) and the library DB's `lib_meta` table.

## Section 3: Agent UX — Discovery, Nudging & Auto-Index

### Layer 1: Auto-discovery (wire up `tag_external_path`)

Currently dead code. Wire into `goto_definition` and `hover` — when LSP resolves to a path outside the project root:

1. Check `LibraryRegistry` — if already registered, done
2. Run `discover_library_root(path)` — finds manifest, extracts name/language
3. Register in `libraries.json` with `indexed: false`
4. Read version from lockfile via `resolve_dependency_versions`

Cheap (filesystem only), happens transparently.

### Layer 2: Nudging (proactive hints in tool responses)

When a tool touches a library that's registered but not indexed:

```json
{
  "library_hint": {
    "name": "tokio",
    "status": "not_indexed",
    "hint": "Library 'tokio' discovered but not indexed. Run index_project(scope='lib:tokio') to enable semantic search."
  }
}
```

**Which tools nudge:** `goto_definition`, `find_symbol`, `list_symbols`, `semantic_search`, `hover`.

**Nudge frequency:** Once per library per session, unless `nudge_dismissed: true` in `libraries.json`. When a library's version changes, `nudge_dismissed` resets to `false` (new version = worth nudging again). This prevents nagging across sessions while still surfacing important changes.

### Layer 3: Auto-index (config-gated)

```toml
[libraries]
auto_index = false  # default off — opt-in
```

When `auto_index = true` and a library is discovered:
1. Log to MCP progress: `"Auto-indexing library 'tokio' in background..."`
2. Spawn background task (fire-and-forget, state tracked in `Agent`)
3. On completion, update `libraries.json`

### JVM source fetching

When a JVM library is discovered and path points to compiled classes (no source files):

1. **Detect**: check for `.java`/`.kt` source files vs only `.class` files
2. **Inform**: `"Fetching sources for spring-boot via mvn dependency:sources..."`
3. **Background fetch**: spawn `mvn dependency:sources` or `gradle downloadSources`
4. **On completion**: update library path to sources location, proceed with indexing or nudge

**Timeout:** 5 minutes default, configurable via `[libraries] fetch_timeout_secs = 300` in `project.toml`.

**Failure policy:** No automatic retry. `Failed` state includes stderr output for diagnostics. The agent or user must manually re-trigger. The `index_status` response shows the failure reason.

**Config default:** `auto_fetch_sources = false` (opt-in). Source fetching requires network access, can be slow, and may require authentication for private repositories. The nudge hint explains how to enable it or run the command manually.

```toml
[libraries]
auto_fetch_sources = false   # fetch JVM sources automatically (default: off)
fetch_timeout_secs = 300     # timeout for source fetching commands
```

The agent sees progress via `index_status()`:
```json
{
  "library_hint": {
    "name": "spring-boot",
    "status": "fetching_sources",
    "hint": "Fetching sources for spring-boot (mvn dependency:sources). Use index_status() to check progress."
  }
}
```

## Section 4: Full Navigation for Library Code

### Tools that gain library support (read-only)

| Tool | Library behavior |
|---|---|
| `list_symbols(path)` | Works if path is inside a registered library |
| `find_symbol(pattern, path)` | Search across project + libraries when scope includes libs |
| `find_references(name_path)` | Already works via LSP |
| `goto_definition` | Already works (discovery trigger) |
| `hover` | Already works via LSP |
| `read_file` | Read-only access to library source files |
| `search_pattern` | Regex search across library source when scoped |

### Tools that stay project-only

| Tool | Reason |
|---|---|
| `edit_file`, `create_file` | Never write to dependency source |
| `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol` | Mutation — project only |

### Path security changes

The existing `validate_read_path` function in `src/util/path_security.rs` already handles canonicalization, null byte checks, deny-list matching, and returns `PathBuf`. We extend it by adding library paths to `PathSecurityConfig`:

```rust
// In PathSecurityConfig (already exists):
pub library_roots: Vec<PathBuf>,  // NEW — populated from LibraryRegistry

// In validate_read_path (extend existing logic, don't replace):
// After the project root check fails, before returning error:
if config.library_roots.iter().any(|lib| canonical.starts_with(lib)) {
    return Ok(canonical);  // registered library — read-only access
}
```

This preserves all existing security checks (canonicalization, null bytes, deny-lists) while adding library path acceptance. `PathSecurityConfig.library_roots` is populated from `LibraryRegistry.all()` when constructing the config at tool call time.

Write tools continue using `validate_write_path` which does not check `library_roots`.

### LSP server management

LSP servers are per-language, not per-project. A rust-analyzer instance can serve symbols for any Rust file. `list_symbols` and `find_symbol` use the same LSP infrastructure — they work once the path validation gate is lifted.

## Section 5: Search Across Multiple DBs

### Search flow

```
semantic_search(query="async runtime", scope="all")
  │
  ├─ embed query → [f32; N]
  │
  ├─ open embeddings/project.db → top K results
  ├─ open embeddings/lib/tokio.db → top K results
  ├─ open embeddings/lib/hyper.db → top K results
  │
  └─ merge all results, sort by cosine similarity, return top K overall
```

### Scope → DB mapping

| Scope | DBs opened |
|---|---|
| `project` (default) | `project.db` only |
| `lib:<name>` | `embeddings/lib/<name>.db` only |
| `libraries` | all files in `embeddings/lib/` |
| `all` | `project.db` + all lib DBs |

### New function

```rust
// src/embed/index.rs
pub fn search_multi_db(
    project_root: &Path,
    query_embedding: &[f32],
    limit: usize,
    scope: &Scope,
    library_registry: &LibraryRegistry,
) -> Result<Vec<SearchResult>>
```

1. Determine which DB files to open based on scope
2. For each DB, run the existing `search_scoped` (or `search` for lib DBs which have no source column) with `limit` per DB — this over-fetches, then the merge step selects the global top-K
3. Collect all results, sort by score descending, truncate to `limit`
4. Each result carries its `source` field: `"project"`, `"lib:tokio"`, etc. (matching existing `Scope` string representation)

**Over-fetch bound:** Each DB returns at most `limit` results. With 10 libraries + project, worst case is `11 * limit` candidates in memory before truncation. For `limit=10` this is 110 rows — negligible. For `limit=100` with 10 libs this is 1100 rows — still acceptable. No additional cap needed for v1 given the 5-10 library assumption.

### Performance

- SQLite connections are cheap (~1ms)
- Per-DB search is faster (smaller DB than combined)
- Typical: 5-10 library DBs
- Connection caching deferred — open/close per search for v1
- Future optimization: SQLite `ATTACH DATABASE` could query multiple DBs in a single connection, but needs sqlite-vec compatibility testing first

### Staleness check during search

While DBs are open, check `lib_meta.version_indexed` against `libraries.json` version. Return stale hints in batch.

## Section 6: Background Indexing Pipeline

### State machine

```
Idle → Fetching Sources → Indexing → Done
         (JVM only)         ↓
                          Failed
```

### Per-library state tracking

```rust
pub enum LibraryIndexState {
    Idle,
    FetchingSources { command: String },
    Indexing { done: usize, total: usize },
    Done { chunks: usize, version: String },
    Failed(String),  // includes stderr output for diagnostics
}
```

Stored in `HashMap<String, LibraryIndexState>` on `Agent`. Separate from project-level `IndexingState` — library and project indexing can run concurrently.

### Pipeline function

```rust
async fn index_library_pipeline(
    root: &Path,
    entry: &LibraryEntry,
    force: bool,
) -> Result<IndexReport>
```

1. Check source availability — scan for source files matching library language
2. If no sources (JVM): detect build tool, set `FetchingSources` state, run fetch command with timeout, update `LibraryEntry.path`
3. Index — create/open `embeddings/lib/<name>.db`, chunk, embed, insert
4. Finalize — write version to `lib_meta`, update `libraries.json`

### Embedding concurrency control

Library and project indexing share the `RemoteEmbedder`. To avoid exhausting API rate limits or connection pools when multiple libraries index in parallel:

- A `tokio::sync::Semaphore` with 2 permits gates embedding API calls
- Both project indexing and library indexing acquire a permit before calling `Embedder::embed()`
- This limits concurrent embedding requests to 2, regardless of how many indexing tasks are running
- The semaphore lives on `Agent` alongside the embedder

### Status reporting via `index_status`

```json
{
  "project": { "status": "indexed", "files": 230, "chunks": 1450 },
  "libraries": {
    "tokio": { "status": "indexed", "version": "1.38.0", "chunks": 820 },
    "serde": { "status": "indexing", "progress": "50/200" },
    "spring-boot": { "status": "fetching_sources" }
  }
}
```

## Section 7: Migration & Backward Compatibility

### Migration trigger

`open_db` detects `embeddings.db` at old location and no `embeddings/project.db`.

### Migration steps

1. Create `embeddings/` and `embeddings/lib/` directories
2. Rename `embeddings.db` → `embeddings/project.db`
3. Query `project.db` for distinct `source` values starting with `lib:`
4. **Fallback for missing source tags:** If no `lib:*` sources are found but `libraries.json` has registered libraries, check the `file_path` column in `code_chunks` against registered library paths. Chunks whose `file_path` starts with a library's `path` are attributed to that library.
5. For each identified library:
   - Create `embeddings/lib/<name>.db` with same schema
   - Copy rows from `code_chunks` + `chunk_embeddings` + `file_hashes`
   - Create `lib_meta` table
   - DELETE those rows from `project.db`
6. VACUUM `project.db`
7. Log migration message

If no library chunks exist (most users today): just rename + create empty `lib/`. Migration is instant.

### Failure handling

The rename in step 2 is atomic on most filesystems. Library extraction in steps 3-6 is additive — partial failure leaves some library chunks in `project.db`, re-extracted on next attempt.

### Backward compatibility

- Old codescout + new layout: `embeddings.db` not found → treats as no index → safe degradation
- New codescout + old layout: triggers migration automatically
- `version`, `version_indexed`, `db_file`, `nudge_dismissed` fields in `libraries.json` are optional — old entries without them use defaults (no staleness check, auto-generated filename, nudge enabled)

### Config additions

```toml
[libraries]
auto_index = false           # auto-index on discovery (default: off)
auto_fetch_sources = false   # fetch JVM sources automatically (default: off)
fetch_timeout_secs = 300     # timeout for source fetching commands

[libraries.version_overrides]
# Manual version pins for ecosystems without lockfile parsing
# spring-boot = "3.2.1"
```

## Section 8: Changes by Module

| Module | Changes |
|---|---|
| `src/embed/index.rs` | `db_path` → `embeddings/project.db`; new `lib_db_path`; `search_multi_db`; migration in `open_db`; `lib_meta` table; `build_library_index` writes separate DB |
| `src/library/registry.rs` | `LibraryEntry` gains `version`, `version_indexed`, `db_file`, `nudge_dismissed`; new `stale_libraries()`; filename sanitization |
| `src/library/versions.rs` | **New** — lockfile parsing per ecosystem, `resolve_dependency_versions` |
| `src/library/discovery.rs` | JVM source detection (`has_sources`, `find_sources_jar`) |
| `src/library/mod.rs` | Add `versions` module |
| `src/util/path_security.rs` | `PathSecurityConfig` gains `library_roots: Vec<PathBuf>`; `validate_read_path` checks library roots |
| `src/agent.rs` | `library_index_states`, `nudged_libraries`, `embedding_semaphore`, `index_library_pipeline` |
| `src/tools/symbol.rs` | Wire `tag_external_path` into `goto_definition`/`hover`; populate `library_roots` in path security config |
| `src/tools/file.rs` | `read_file`, `search_pattern` accept library paths via extended `PathSecurityConfig` |
| `src/tools/semantic.rs` | `search_multi_db`; staleness hints; `index_status` reports per-library states |
| `src/tools/library.rs` | `list_libraries` shows version comparison; `IndexLibrary` uses pipeline |
| `src/tools/config.rs` | `project_status` includes library index summary |
| `src/config/mod.rs` | `[libraries]` section in `ProjectConfig` |
| `src/prompts/server_instructions.md` | Library navigation guidance, scope semantics |
| `src/prompts/onboarding_prompt.md` | Library discovery and indexing mention |
| `src/tools/workflow.rs` | `build_system_prompt_draft` includes library info |

## Section 9: Test Strategy

### Migration tests

- Old layout detected → project.db created, lib/ directory created
- Library chunks with `source = "lib:*"` extracted to separate DBs
- Fallback: library chunks identified by `file_path` when `source` column is missing
- Partial migration failure → next attempt completes extraction
- No library chunks → instant migration (rename only)

### Multi-DB search tests

- `scope="project"` searches only `project.db`
- `scope="lib:tokio"` searches only `tokio.db`
- `scope="all"` merges results from all DBs, sorted by score
- Results from different DBs interleave correctly by score
- Missing library DB (not indexed yet) → graceful skip, not error

### Version staleness tests

- `version != version_indexed` → stale hint in `semantic_search` response
- `version == version_indexed` → no hint
- Version refresh after `index_project(scope="lib:<name>")` updates both fields
- Missing version (old `libraries.json` format) → no staleness check

### Path security tests

- Library path accepted by `validate_read_path` when registered
- Library path rejected by `validate_write_path` (always)
- Unregistered external path rejected by both
- Library path still goes through canonicalization and deny-list checks

### Discovery and nudging tests

- `goto_definition` landing outside project root → library registered
- Nudge appears once per library per session
- `nudge_dismissed: true` → no nudge
- Version change resets `nudge_dismissed` to `false`

### Filename sanitization tests

- Scoped npm package `@scope/name` → `@scope--name.db`
- Collision detection and hash suffix
- Case insensitivity on macOS/Windows

## Out of Scope

- Global shared cache across projects (deferred for multi-project scenario)
- New external dependencies — same SQLite + sqlite-vec engine
- Changes to embedding model or chunking strategy
- Write tool access to library code
- Auto-indexing by default (opt-in only)
- Gradle-without-lockfile version parsing (deferred — DSL parsing too complex)
