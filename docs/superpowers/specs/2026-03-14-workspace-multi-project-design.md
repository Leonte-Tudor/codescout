# Workspace Multi-Project Support

**Date**: 2026-03-14
**Status**: Draft (reviewed, issues addressed)
**Scope**: Architecture design for multi-project workspace support in codescout

---

## Problem

codescout assumes one project root per server instance. Every tool inherits this through
`require_project_root()` → single `ActiveProject`. This breaks down for multilanguage
repositories like `backend-kotlin` which contains:

| Language | Files | Purpose | Root |
|----------|-------|---------|------|
| Kotlin | 581 `.kt` | Main backend (Ktor + OptaPlanner) | `.` |
| TypeScript | 1,418 `.ts` | MCP server | `mcp-server/` |
| Python | 5,262 `.py` | Chat service, AI services | `python-services/` |

**Symptoms:**
- Onboarding reports "Languages: kotlin, typescript, python" but doesn't map which language
  lives where or that each has its own build tool, entry points, and test setup
- LSP servers get the wrong workspace root (TypeScript LSP gets repo root instead of
  `mcp-server/` where `tsconfig.json` lives)
- Semantic search returns mixed results across all languages with no scoping
- System prompt draft mentions languages but not subsystem structure
- `run_command` has no awareness that `./gradlew build` and `npm test` need different working
  directories

**Scale requirement**: Must handle CI monorepos with 50+ services without spawning 50 LSP
servers or requiring manual configuration.

---

## Approach

**Workspace model with lazy-activated projects** — one codescout MCP server, a new `Workspace`
layer above `ActiveProject`. Projects are auto-discovered from build manifests, lazily
activated on first file touch.

```
Workspace (repo root)
├── Project "backend-kotlin" (kotlin)     ← activated (focused)
├── Project "mcp-server" (typescript)     ← dormant
├── Project "python-services" (python)    ← dormant
└── ... N more services                   ← dormant
```

Key properties:
- **Awareness** of all projects is cheap (metadata from manifest walk)
- **Activation** (LSP + memory + config) is expensive and on-demand
- **Focus** determines the default project for tools that don't specify one
- **Resource caps** prevent runaway LSP server spawning

Research basis: VS Code multi-root workspaces, IntelliJ load/unload model, LSP
`workspaceFolders` spec, Nx/Bazel project discovery, MCP roots specification.

---

## Design

### 1. Core Data Model

```rust
struct Workspace {
    root: PathBuf,                          // repo root
    config: WorkspaceConfig,                // .codescout/workspace.toml
    projects: Vec<Project>,                 // discovered sub-projects
    focused: Option<String>,                // project ID (not index — stable across mutations)
}

struct Project {
    id: String,                             // short name: "mcp-server", "python-services", "root"
    relative_root: PathBuf,                 // relative to workspace root
    languages: Vec<String>,                 // detected languages
    manifest: Option<String>,               // "package.json", "build.gradle.kts", etc.
    state: ProjectState,
}

enum ProjectState {
    Dormant,                                // metadata only, no LSP, no index
    Activated {                             // LSP running, index available
        config: ProjectConfig,
        memory: MemoryStore,
        private_memory: MemoryStore,
        library_registry: LibraryRegistry,
        dirty_files: Arc<Mutex<HashSet<PathBuf>>>,
    },
}
```

**Replaces `ActiveProject`**: Today's `ActiveProject` maps to `ProjectState::Activated`.
`AgentInner.active_project: Option<ActiveProject>` becomes `AgentInner.workspace: Option<Workspace>`.

**Focused project uses `Option<String>` (project ID)**, not an index into the `Vec<Project>`.
This is stable across vector mutations (sorting, filtering, removal via config edits). Lookup
is by ID scan — negligible cost for small vectors.

**Project resolution**: Tools can pass `project: "mcp-server"` to target a specific project.
If omitted, the focused project is used. File paths auto-resolve using **longest prefix match**:
`mcp-server/src/index.ts` → project "mcp-server" because `mcp-server/` is the most specific
project root that contains that path. Files not claimed by any sub-project fall to the root
project (the catch-all).

**Backward compatibility**: Single-project repos have one project (id: workspace dir name),
always focused and activated. No config changes needed. Absence of `workspace.toml` means
single-project mode.

### 2. Project Discovery

**Manifest walk**: On onboarding, walk the workspace root (max depth configurable, default 3,
respecting `.gitignore`) for known build manifests:

```
Cargo.toml          → Rust
package.json        → TypeScript/JavaScript (only if has "scripts" or "main"/"module")
pyproject.toml      → Python
build.gradle.kts    → Kotlin/Java
build.gradle        → Kotlin/Java
go.mod              → Go
pom.xml             → Java
CMakeLists.txt      → C/C++
mix.exs             → Elixir
Gemfile             → Ruby
setup.py            → Python (legacy)
requirements.txt    → Python (only if no pyproject.toml sibling)
```

**Rules:**
- Root project is always first (if workspace root has a manifest)
- Skip nested manifests within same-language parent projects (e.g., `node_modules/`)
- Project ID derived from directory name; collisions resolved by appending relative path

**Applied to `backend-kotlin`:**
```
./build.gradle.kts              → Project "backend-kotlin" (kotlin)
./mcp-server/package.json       → Project "mcp-server" (typescript)
./python-services/requirements.txt → Project "python-services" (python)
```

**Config override**: `.codescout/workspace.toml` can exclude false positives, add manual
project definitions, and declare `depends_on` relationships (informational, for system prompt).

```toml
[workspace]
name = "backend-kotlin"
discovery_max_depth = 3          # configurable walk depth (default 3)

[resources]
max_lsp_clients = 5
idle_timeout_secs = 600

exclude_projects = ["node_modules", "build"]

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["backend-kotlin"]  # list of project IDs
```

**`depends_on` schema**: A list of project IDs. Purely informational — rendered in the system
prompt and onboarding output to help the agent understand cross-project relationships. Does
not affect tool behavior.

**When discovery runs**: First onboarding (full walk, writes config), subsequent sessions
(reads config), `onboarding(force=true)` (re-walks, merges with manual edits).

**Per-project context gathering**: `gather_project_context` is called once per discovered
project (with the project's root as the base), not once for the whole workspace. Each project
gets its own `entry_points`, `test_dirs`, `build_file_name`, and `readme_path`. The workspace
onboarding output assembles these into a per-project table.

### 3. LSP Manager Changes

**Current state (important nuance)**: The `LspManager.clients` map is keyed by language string
only (`HashMap<String, Arc<LspClient>>`). However, `get_or_start` already receives
`workspace_root` and checks it:

```rust
if client.is_alive() && client.workspace_root == workspace_root {
    return Ok(client.clone());
}
```

If the root doesn't match, the code falls through to `do_start` which **silently overwrites**
the existing client for that language. This is a **latent bug**: calling
`get_or_start("typescript", "mcp-server/")` after `get_or_start("typescript", "root/")`
replaces the root's TypeScript LSP without warning.

**New**: Keyed by `(language, project_root)`:

```rust
#[derive(Hash, Eq, PartialEq, Clone)]
struct LspKey {
    language: String,
    project_root: PathBuf,
}

// LspManager
clients: Mutex<HashMap<LspKey, Arc<LspClient>>>
```

This fixes the overwrite bug and enables per-project LSP servers simultaneously.

**Root resolution**: Instead of `require_project_root()`, tools use the file being operated on
to resolve the correct project root (longest prefix match against known projects). `find_symbol`
on `mcp-server/src/tools.ts` automatically routes to TypeScript LSP rooted at `mcp-server/`.

**Resource management:**
- **Concurrent cap**: Max 5 activated LSP clients (configurable)
- **LRU eviction**: When cap hit, shut down least-recently-used client gracefully
- **Idle timeout**: Clients unused for 10 minutes are shut down proactively

**Cost of re-activation**: JVM servers (kotlin-lsp): 60-300s cold start. Lightweight servers
(typescript-language-server, pyright): <5s. Acceptable because you rarely bounce between more
than 2-3 projects in a conversation.

**`notify_file_changed` scoping**: In multi-project mode, `did_change` notifications are sent
only to LSP clients whose project root contains the changed file. Broadcasting to all clients
is harmless but wasteful — scoping avoids unnecessary work, especially at scale.

**Activation concurrency guard**: When two tool calls race to activate the same dormant project,
a barrier/dedup mechanism (similar to the existing `LspManager.starting` watch-channel pattern)
ensures only one activation proceeds. The second caller waits and reuses the result.

### 4. Semantic Search & Embedding Index

**Single index, project-tagged chunks:**

```sql
ALTER TABLE chunks ADD COLUMN project_id TEXT DEFAULT 'root';
```

- One `embeddings.db` for the workspace — chunks tagged with `project_id`
- Cross-project search works by default (no filter)
- Project-scoped search via `WHERE project_id = ?`
- `build_index` resolves `project_id` from file path during walk

**Relationship with existing `source` column**: The `chunks` table already has
`source TEXT NOT NULL DEFAULT 'project'` which distinguishes project chunks from library
chunks (`source = 'lib:<name>'`). The new `project_id` column is orthogonal:

| `source` | `project_id` | Meaning |
|----------|-------------|---------|
| `'project'` | `'mcp-server'` | Project chunk from the mcp-server sub-project |
| `'project'` | `'root'` | Project chunk from the root project (or single-project mode) |
| `'lib:tokio'` | `NULL` | Library chunk — lives in a separate library DB, not the main index |

Library chunks are stored in per-library databases (`lib_db_path`), not in the workspace
`embeddings.db`. The `project_id` column only applies to project chunks. Library DBs are
unchanged.

**Query-time scoping** via extended `scope` parameter:
```
scope: "project"              → focused project only
scope: "project:mcp-server"   → specific project
scope: "all"                  → all projects (default for workspaces)
scope: "lib:tokio"            → library (unchanged)
```

### 5. Tool Surface Changes

**Principle**: Most tools gain an optional `project` parameter, defaulting to focused project.
File-path-based tools auto-resolve from the path.

**Resolution helper (centralized):**
```rust
async fn resolve_root(&self, project: Option<&str>, file_hint: Option<&Path>) -> Result<PathBuf> {
    match (project, file_hint) {
        (Some(id), _) => self.project_root_by_id(id),
        (None, Some(path)) => Ok(self.resolve_project_from_path(path)),  // longest prefix match
        (None, None) => self.focused_project_root(),
    }
}
```

**Default scoping rationale**: `semantic_search` defaults to all projects because it's
concept-based and benefits from breadth ("how does authentication work" may span projects).
`find_symbol` (pattern, no path) defaults to focused project because symbol names are
language-specific and cross-project results are usually noise. This asymmetry is intentional.

**Navigation tools** (implicit resolution from file path):
- `find_symbol`, `list_symbols`, `goto_definition`, `hover`, `find_references`
- File path determines project + LSP — no `project` parameter needed

**Search tools** (explicit scoping valuable):
- `semantic_search`: all projects by default, `project` param to filter
- `search_pattern`: all projects by default, `project` param scopes to project root
- `find_symbol` (pattern, no path): focused project by default, `project: "all"` for cross-project

**File tools** (path determines everything):
- `read_file`, `edit_file`, `create_file`, `list_dir`, `find_file`
- Path resolved against workspace root. Auto-activates dormant projects.

**Workflow tools:**
- `run_command`: `cwd` defaults to focused project root. `project` param overrides.
- `onboarding`: discovers workspace + all projects, reports per-project structure
- `activate_project`: see disambiguation rules below
- `project_status`: structured per-project table (ID, language, state, LSP status, index %)

**`activate_project` disambiguation**: The argument can be an absolute path (workspace
init/switch) or a project ID (focus switch). Rule: if the argument matches a known project ID
in the current workspace, treat as focus switch. If it looks like a path (contains `/` or
starts with `.` or is absolute), treat as workspace init. If ambiguous, prefer project ID.

**Memory tools:**
- `memory(write, topic)`: writes to focused project's memory
- `memory(write, topic, project: "mcp-server")`: writes to specific project's memory
- `memory(recall, query)`: searches across all project memories
- Workspace-level memories for cross-cutting knowledge (architecture, domain glossary)
- Project-level memories for project-specific knowledge (conventions, gotchas)

### 6. Configuration & Storage Layout

```
.codescout/
├── workspace.toml              # workspace config (multi-project only)
├── project.toml                # root project config (always exists)
├── embeddings.db               # single unified index (project_id tagged)
├── usage.db                    # usage stats (workspace-level)
├── memories/                   # workspace-level memories
│   ├── onboarding.md
│   ├── architecture.md
│   └── ...
└── projects/                   # per-project state (multi-project only)
    ├── mcp-server/
    │   ├── project.toml        #   project-specific config overrides
    │   └── memories/
    │       ├── onboarding.md
    │       └── conventions.md
    └── python-services/
        ├── project.toml
        └── memories/
```

**Config inheritance — merge strategy:**
```
workspace.toml → project.toml → projects/<id>/project.toml
```
- **Arrays** (e.g., `ignored_paths`, `languages`): merged (union of all levels)
- **Scalars** (e.g., `embedding_model`, `chunk_size`, `tool_timeout_secs`): last-writer-wins
  (most specific config takes precedence: per-project > root project > workspace)
- **Maps/tables** (e.g., `[security]`): deep merge (per-key override)

**Migration**: Existing single-project repos need zero changes. Running `onboarding(force=true)`
on a multi-project repo creates `workspace.toml` and `projects/` directories.

---

## Implementation Phases

### Phase 1: Workspace Discovery & Multi-Project Onboarding
**Value**: Onboarding sees all sub-projects, system prompt reflects real structure.

- Manifest walk discovers sub-projects
- `gather_project_context()` called per-project (with sub-root)
- `GatheredContext` gains `projects: Vec<DiscoveredProject>` with per-project entry points,
  test dirs, build files
- Onboarding output includes per-project table in system prompt draft
- Create `workspace.toml` on first multi-project onboarding
- `project_status` reports discovered projects

**Scope**: ~500 lines, low risk, purely additive.

### Phase 2: Project-Tagged Embedding Index
**Value**: `semantic_search` can scope by project.

- Add `project_id TEXT` column to `chunks` table (with migration, default `'root'`)
- `build_index` tags chunks during walk using project path resolution
- `semantic_search`, `search_pattern`, `find_symbol` gain `project` parameter
- `scope: "project:name"` syntax

**Scope**: ~300 lines, medium risk (schema migration).

### Phase 3+4: Workspace-Aware Agent Model + LSP Re-Keying (ship together)
**Value**: Multiple projects activated simultaneously, per-project LSP servers.

These phases must ship together. Phase 3 alone (multi-project activation without LSP re-keying)
would trigger the latent LSP client overwrite bug: two activated projects with the same language
would race to replace each other's LSP client in the single-language-keyed map.

**Phase 3 changes:**
- New `Workspace`, `Project`, `ProjectState` types
- `AgentInner.workspace` replaces `active_project`
- Centralized `resolve_root()` helper
- All 24 `require_project_root()` call sites updated
- `activate_project` dual role with disambiguation
- Per-project memory directories + config inheritance
- Activation concurrency guard (watch-channel barrier per project ID)

**Phase 4 changes:**
- `LspKey { language, project_root }` replaces `String` key
- LRU eviction with configurable cap
- Idle timeout background task
- Barrier/dedup mechanism re-keyed on `LspKey`
- `notify_file_changed` scoped to relevant project's LSP clients

**Combined scope**: ~1200 lines, high risk (core agent model + LSP lifecycle).

---

## Decisions (from Open Questions)

1. **`find_symbol` cross-project default**: Focused project. Overflow hint mentions other
   projects exist. Rationale: symbol names are language-specific; cross-project results add
   noise. `semantic_search` defaults to all because it's concept-based and benefits from breadth.

2. **Dormant project activation UX**: Transparent but announced. First tool response after
   lazy activation includes a hint: "Activating project mcp-server — JVM-based language
   servers may take up to 5 minutes for initial startup." Non-JVM projects activate silently.

3. **Multi-language projects**: `languages` is a list. Both languages get LSP servers keyed
   to the same project root. E.g., a project with `.kt` and `.java` files gets both
   `kotlin-lsp` and `jdtls` rooted at the same directory.

4. **Root project**: Always exists as a catch-all. Uses longest-prefix-match: files claimed
   by a more specific sub-project go there; unclaimed files (e.g., `scripts/deploy.sh` at
   workspace root) fall to the root project. The root project may have no manifest and no
   language — it still serves as the workspace-level container.
