# Library Search Design

Search and navigate third-party library/dependency source code through code-explorer. Read-only access to library code via LSP-inferred discovery, symbol navigation, and semantic search.

## Approach

**Library Registry** (Approach 1) — libraries as a first-class concept alongside the project. A `LibraryRegistry` tracks discovered library paths. Tools gain an optional `scope` parameter. Embedding index distinguishes project vs library chunks. All library access is read-only.

**Progressive rollout in 4 levels:**

| Level | Name | Summary |
|-------|------|---------|
| A | Follow-through reads | Read files LSP points to outside project root |
| B | Symbol navigation | `find_symbol` / `get_symbols_overview` work on library code |
| C | Semantic search | Explicit `index_library` + scoped `semantic_search` |
| D | LSP-inferred discovery | Auto-register libraries from `goto_definition` responses |

## Library Registry

Core data structure managing known library locations.

### Data Model

```rust
struct LibraryEntry {
    name: String,                   // "serde", "tokio", "requests"
    version: Option<String>,        // "1.0.210" (best-effort from manifest)
    path: PathBuf,                  // absolute path to library root
    language: String,               // "rust", "python", etc.
    discovered_via: DiscoveryMethod,
    indexed: bool,                  // embedding index built
}

enum DiscoveryMethod {
    LspFollowThrough,
    Manual,
}
```

### Persistence

`.code-explorer/libraries.json` — JSON array. Loaded at startup, updated on discovery.

### API

- `register(name, path, language, method)` — add or update
- `lookup(name) -> Option<&LibraryEntry>` — find by name
- `all() -> &[LibraryEntry]` — list all
- `resolve_path(name, relative) -> PathBuf` — resolve path within a library
- `is_library_path(absolute_path) -> Option<&LibraryEntry>` — check if path belongs to a known library

## Path Security Changes

### Read Path Validation

Extended flow for `validate_read_path`:

1. If relative → resolve against `project_root` (existing)
2. If absolute:
   - Inside `project_root`? → allow (existing)
   - Inside a registered library path? → allow, read-only
   - In deny-list? → reject (existing)
   - Otherwise → reject

Library paths are only allowed after registration in the `LibraryRegistry`. Security surface grows incrementally, not all-at-once.

### Write Path Validation

No changes. `validate_write_path` only allows project root. Attempts to edit library files return: `"library files are read-only"`.

## Level A: Follow-Through Reads

### goto_definition Enrichment

When result URI is outside project root:
- If already registered → tag with `"source": "lib:<name>"`
- If not registered → auto-discover (walk up to manifest), register, then tag

### read_file Accepts Library Paths

Absolute paths returned by `goto_definition` are allowed by the updated path security. No new parameters needed.

### Result Tagging

Any tool result with a file path outside project root gets `"source": "lib:<name>"` in the JSON output.

### UX Flow

```
LLM: find_symbol("Deserialize", include_body: true)
→ finds use in project code

LLM: goto_definition on Deserialize at src/models.rs:5:12
→ returns /home/user/.cargo/registry/.../serde-1.0.210/src/de.rs:123
→ auto-registers "serde" library
→ result tagged: { "uri": "...", "source": "lib:serde", "library_path": "src/de.rs" }

LLM: read_file("/home/user/.cargo/registry/.../serde-1.0.210/src/de.rs", start_line: 120, end_line: 180)
→ works because serde is registered
```

## Level B: Symbol Navigation in Libraries

### Scope Parameter

Added to 4 read-only tools:

| Tool | Scope values | Default |
|------|-------------|---------|
| `find_symbol` | `"project"`, `"libraries"`, `"all"`, `"lib:<name>"` | `"project"` |
| `get_symbols_overview` | same | `"project"` |
| `find_referencing_symbols` | same | `"project"` |
| `list_functions` | same | `"project"` |

### Scope Behavior

- `"project"` — existing behavior
- `"lib:<name>"` — operate on that library's root; LSP started with library path as workspace root
- `"libraries"` — all registered libraries
- `"all"` — project + all libraries

### Result Tagging

Every symbol gets `"source": "project"` or `"source": "lib:<name>"`.

### LSP Consideration

`LspManager` already keys by `(language, workspace_root)`. A library-scoped request starts a separate LSP instance rooted at the library. Only triggered on explicit `scope` request.

### Write Tools Excluded

`replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol`, `rename_symbol` remain project-only. No `scope` parameter.

## Level C: Semantic Search in Libraries

### New Tool: index_library

```json
{
  "name": "index_library",
  "params": {
    "name": "serde",
    "force": false
  }
}
```

Library must be in the registry. Calls `build_index` scoped to the library path.

### Schema Change

```sql
ALTER TABLE chunks ADD COLUMN source TEXT NOT NULL DEFAULT 'project';
-- values: 'project', 'lib:serde', 'lib:tokio', etc.
```

### semantic_search Scope

Same `scope` parameter as symbol tools:
- `"project"` (default) — `WHERE source = 'project'`
- `"lib:<name>"` — `WHERE source = 'lib:<name>'`
- `"libraries"` — `WHERE source != 'project'`
- `"all"` — no filter

### index_status Extension

Reports per-source stats:

```json
{
  "project": { "files": 42, "chunks": 310 },
  "lib:serde": { "files": 18, "chunks": 95 },
  "total": { "files": 60, "chunks": 405 }
}
```

### Index Lifecycle

- Persists across sessions in `embeddings.db`
- `libraries.json` tracks `indexed: true/false` per entry
- Stale paths (library removed/upgraded) reported by `index_status`
- Shared embedding model — model change invalidates all indexes

## Level D: LSP-Inferred Discovery

### Triggers

Any LSP response containing a URI outside `project_root`:
- `goto_definition`
- `find_referencing_symbols`
- `rename_symbol` response (locations seen, edits not applied)

### Discovery Algorithm

```
on_external_uri(uri):
  1. Convert URI to absolute path
  2. If inside a registered library → done (tag result)
  3. Walk parent directories for manifest:
     - Cargo.toml → Rust crate
     - package.json → npm package
     - setup.py / pyproject.toml → Python package
     - go.mod → Go module
     - pom.xml / build.gradle.kts → JVM
  4. If manifest found:
     - Extract name + version (best-effort regex)
     - Register with DiscoveryMethod::LspFollowThrough
  5. If no manifest found:
     - Use parent directory name as library name
     - Register with version: None
```

### Manifest Parsing

Best-effort regex extraction, not full parsing:
- Cargo.toml: `name = "serde"`, `version = "1.0.210"`
- package.json: `"name": "lodash"`, `"version": "4.17.21"`
- pyproject.toml: `name = "requests"`, `version = "2.31.0"`

### Performance

Discovery runs at most once per unique parent directory. Registry acts as cache.

### No Eager Crawling

Never scan `~/.cargo/registry/` or `node_modules/` proactively. Only discover libraries the LLM navigates to.

## New Tools Summary

| Tool | Level | Description |
|------|-------|-------------|
| `index_library` | C | Build embedding index for a registered library |
| `list_libraries` | A | Show all registered libraries and their status |

## Modified Tools Summary

| Tool | Change | Level |
|------|--------|-------|
| `read_file` | Accepts absolute library paths | A |
| `goto_definition` (internal) | Auto-discovers + tags library results | A/D |
| `find_symbol` | `scope` parameter | B |
| `get_symbols_overview` | `scope` parameter | B |
| `find_referencing_symbols` | `scope` parameter | B |
| `list_functions` | `scope` parameter | B |
| `semantic_search` | `scope` parameter | C |
| `index_status` | Per-source breakdown | C |
