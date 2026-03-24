# activate_project Output Optimization

**Date:** 2026-03-21
**Status:** Draft
**Branch:** experiments

## Problem

The `activate_project` tool returns the same verbose output regardless of activation mode (read-only vs read-write). This wastes tokens and misses the opportunity to surface mode-appropriate guidance.

**Current issues:**

1. **Full config dump on every activation** — includes internal settings the LLM never acts on (`semantic_anchor_min_similarity`, `shell_output_limit_bytes`, `encoding`, float serialization noise like `0.30000001192092896`).
2. **No mode differentiation** — RO activations (browsing a library or sub-project) include security settings; RW activations don't promote `project_status` for health checks.
3. **Missing orientation info** — available memories, workspace siblings, and semantic index status are absent — forcing separate `project_status` or `memory(list)` calls.
4. **Focus-switch path returns minimal info** — switching to a sub-project by ID gives only `project_root` and a hint, losing all orientation context.

## Design

### Approach: Shared Core + Mode-Specific Extras

A slim shared core present in both RO and RW responses, with mode-appropriate additions.

### Shared Core (both modes)

| Field | Type | Source | Purpose |
|-------|------|--------|---------|
| `status` | `"ok"` | — | Success indicator |
| `project` | string | `config.project.name` | Project name |
| `project_root` | string | `ActiveProject.root` | Absolute path (always absolute, never relative) |
| `read_only` | bool | `ActiveProject.read_only` | Current access mode |
| `languages` | string[] | `config.project.languages` | What's in the project |
| `index` | object | `embed::index::project_db_path` | `{ "status": "indexed" \| "not_indexed", "hint": "..." }` |
| `memories` | string[] | `memory.list()` | Available shared memory topics (graceful: `[]` on error) |
| `workspace` | object[] \| null | `Workspace.projects` | `[{ "id", "root", "languages", "depends_on" }]` or null for single-project |
| `auto_registered_libs` | object \| null | `auto_register_deps` | `{ "count": N, "without_source": M }` — only when libs were registered |
| `hint` | string | — | Mode-appropriate guidance |

**Notes:**
- `memories` lists shared memory topics only. Private memories are excluded — they are machine-local and not useful for orientation.
- `workspace[].root` is relative to the workspace root (e.g. `tests/fixtures/java-library`), but `project_root` at the top level is always absolute.
- `workspace[].depends_on` is a `string[]` of project IDs this project depends on (from `workspace.toml` config). Empty array if none.
- All fallible data sources (`memory.list()`, index check) use graceful degradation (`unwrap_or_default()` / `false`), matching the pattern in `Agent::project_status`.

### RW-Only Extras

| Field | Type | Source | Purpose |
|-------|------|--------|---------|
| `security_profile` | string | `config.security.profile` | "default" or "root" |
| `shell_enabled` | bool | `config.security.shell_enabled` | Can LLM run commands? |
| `github_enabled` | bool | `config.security.github_enabled` | Are GitHub tools available? |

### RO-Only Extras

None beyond the shared core. The `hint` carries the "remember to switch back" warning.

### Hint Content by Scenario

| Scenario | Hint |
|----------|------|
| **First activation (RW, home)** | `"CWD: {root}. Run project_status() for health checks and memory staleness."` |
| **Return to home (RW)** | `"Returned to home project. CWD: {root}. Run project_status() to check memory staleness."` |
| **Switch away (RW, non-home)** | `"Switched project (read-write). CWD: {root} — remember to activate_project(\"{home}\") when done."` |
| **Switch away (RO, non-home)** | `"Browsing {name} (read-only). CWD: {root} — remember to activate_project(\"{home}\") when done."` |
| **Focus switch by ID (any)** | Same as "switch away" for the resolved mode |

**Note:** The `project_status()` nudge in the RW hints is a new behavioral addition. Currently, hints for home activations don't mention `project_status` at all. This is the primary mechanism for promoting `project_status` usage.

### Example Outputs

**RW activation (home project, session start):**
```json
{
  "status": "ok",
  "project": "code-explorer",
  "project_root": "/home/marius/work/claude/code-explorer",
  "read_only": false,
  "languages": ["rust", "typescript", "python"],
  "index": { "status": "not_indexed", "hint": "Run index_project() to enable semantic_search." },
  "memories": ["architecture", "conventions", "gotchas", "language-patterns"],
  "workspace": [
    { "id": "java-library", "root": "tests/fixtures/java-library", "languages": ["kotlin", "java"], "depends_on": [] },
    { "id": "rust-library", "root": "tests/fixtures/rust-library", "languages": ["rust"], "depends_on": [] }
  ],
  "security_profile": "default",
  "shell_enabled": true,
  "github_enabled": false,
  "hint": "CWD: /home/marius/work/claude/code-explorer. Run project_status() for health checks and memory staleness."
}
```

**RO activation (sub-project browsing via full path):**
```json
{
  "status": "ok",
  "project": "java-library",
  "project_root": "/home/marius/work/claude/code-explorer/tests/fixtures/java-library",
  "read_only": true,
  "languages": ["kotlin", "java"],
  "index": { "status": "not_indexed", "hint": "Run index_project() to enable semantic_search." },
  "memories": [],
  "workspace": [
    { "id": "code-explorer", "root": ".", "languages": ["rust", "typescript", "python"], "depends_on": [] },
    { "id": "rust-library", "root": "tests/fixtures/rust-library", "languages": ["rust"], "depends_on": [] }
  ],
  "hint": "Browsing java-library (read-only). CWD: /home/marius/work/claude/code-explorer/tests/fixtures/java-library — remember to activate_project(\"/home/marius/work/claude/code-explorer\") when done."
}
```

**Focus switch by project ID:**
```json
{
  "status": "ok",
  "project": "rust-library",
  "project_root": "/home/marius/work/claude/code-explorer/tests/fixtures/rust-library",
  "read_only": true,
  "languages": ["rust"],
  "index": { "status": "not_indexed", "hint": "Run index_project() to enable semantic_search." },
  "memories": [],
  "workspace": [
    { "id": "code-explorer", "root": ".", "languages": ["rust", "typescript", "python"], "depends_on": [] },
    { "id": "java-library", "root": "tests/fixtures/java-library", "languages": ["kotlin", "java"], "depends_on": [] }
  ],
  "hint": "Browsing rust-library (read-only). CWD: /home/marius/work/claude/code-explorer/tests/fixtures/rust-library — remember to activate_project(\"/home/marius/work/claude/code-explorer\") when done."
}
```

### What's Removed (vs current output)

The entire `config` object is dropped. These fields are no longer in the response:

- `config.project.encoding` — always UTF-8, never actionable
- `config.project.tool_timeout_secs` — internal
- `config.project.system_prompt` — already injected via server instructions
- `config.embeddings.model` — internal detail
- `config.embeddings.drift_detection_enabled` — internal
- `config.ignored_paths` — internal
- `config.security` (full object) — replaced by 3 actionable booleans in RW mode
- `config.memory` (thresholds, anchors) — internal, `project_status` handles staleness
- `config.libraries` (auto_index, fetch config) — internal

### Implementation Notes

#### Data Sources

All required data is already accessible from `ToolContext`:

- **Project name/root/config:** `ctx.agent.with_project(|p| ...)`
- **Memories:** `p.memory.list()` — already used by `project_status`
- **Index status:** `crate::embed::index::project_db_path(&p.root).exists()`
- **Workspace:** `inner.workspace` — already built by `project_status`
- **Libraries:** `auto_register_deps` return value (already captured)

#### Focus-Switch Path: Lazy Activation

Currently, `switch_focus(project_id)` only sets `Workspace.focused` — the target project remains in `ProjectState::Dormant` with no `ActiveProject` (no config, no `MemoryStore`, no index). This means `with_project(|p| ...)` would fail after a focus-switch.

**Solution: promote Dormant → Activated on focus-switch** via a new `Agent::activate_within_workspace(project_id, read_only)` method.

**Why not `Agent::activate()`?** `activate()` calls `discover_projects()` from the new root, which rebuilds the entire workspace rooted at the sub-project — destroying the parent workspace topology and losing all sibling projects. Focus-switch needs to promote a single project in-place.

**New method: `Agent::activate_within_workspace(project_id, read_only)`:**

1. Looks up the project by ID in `inner.workspace.projects`
2. Resolves its absolute root from `workspace.root` + `discovered.relative_root`
3. Loads `ProjectConfig` for the target project
4. Opens `MemoryStore` (shared + private) and `LibraryRegistry`
5. Promotes the project from `Dormant` to `Activated` in-place (mutating `project.state`)
6. Sets `workspace.focused = Some(project_id)`
7. Does **not** rebuild the workspace or call `discover_projects()`

This preserves the workspace topology while enabling `with_project()` access for the full orientation response.

The focus-switch path remains a convenience (accept bare ID instead of full path), but the activation is no longer lightweight. This is acceptable because:
- Focus-switches are infrequent (once per project visit)
- Config loading + MemoryStore init is fast (< 10ms typically)
- The benefit (consistent full output) outweighs the cost

**Auto-registration on focus-switch:** `auto_register_deps` should run on focus-switch (first activation of a sub-project), since the sub-project may have its own dependencies. It is idempotent — re-registering already-known libraries is a no-op. For re-activations of previously-activated projects, the libraries are already registered.

**Implementation:** In the `call` method, replace the current focus-switch early-return with:

```rust
if is_project_id {
    let read_only = optional_bool_param(&input, "read_only");
    ctx.agent.activate_within_workspace(path, read_only).await?;
    // Fall through to shared build_activation_response
}
```

#### Refactoring the `call` Method

The current `call` method has two code paths (focus-switch vs full-activation) that build different response shapes. Both should be unified to call a shared `build_activation_response` helper that:

1. Reads project name, root, languages, read_only from `ActiveProject`
2. Lists memories via `p.memory.list().unwrap_or_default()`
3. Checks index status via `project_db_path(&root).exists()`
4. Builds workspace summary (extract from `project_status` into a shared helper)
5. Conditionally adds RW extras (security_profile, shell_enabled, github_enabled)
6. Builds the hint string based on scenario (first activation, return-to-home, switch-away)

#### Workspace Summary Helper

Extract workspace-building logic from `Agent::project_status` into a reusable method (e.g. `Agent::workspace_summary() -> Option<Vec<WorkspaceProjectSummary>>`) so both `activate_project` and `project_status` can use it without duplication. Include `depends_on` from `workspace.toml`.

#### `format_compact` Update

The compact formatter should be updated to reflect the new shape:

```
activated · code-explorer (rw) · 4 memories · index: not_indexed · 2 workspace projects
```

For RO:
```
activated · java-library (ro) · 0 memories · index: not_indexed · 2 workspace projects
```

Single-project (no workspace):
```
activated · my-project (rw) · 3 memories · index: indexed
```

With auto-registered libs:
```
activated · code-explorer (rw) · 4 memories · index: not_indexed · 2 workspace projects · auto-registered 38 libs (5 without source)
```

The workspace segment is omitted when `workspace` is null (single-project case).

**Change from current:** The current compact format appends the full hint text as the last segment (e.g. `· CWD: /home/user/foo`). The new format drops the hint from the compact line — the hint is still present in the full JSON response and adds no value in the compact summary. The `(rw)`/`(ro)` tag replaces the need for the CWD hint in compact form.

#### Auto-Registered Libs

Change from a full array of `{name, language, source_available}` objects to a summary: `{ "count": N, "without_source": M }`. The full list is rarely useful — the LLM can call `list_libraries` if needed.

#### Prompt Surface Update

Update `src/prompts/server_instructions.md` to reflect that:
- `activate_project` now returns an orientation card (memories, index status, workspace) instead of full config
- `project_status` is the canonical source for detailed health checks (memory staleness, config details)
- This aligns with the Prompt Surface Consistency rule (all 3 surfaces must stay coordinated)

### Tests to Add/Update

1. **`activate_project_rw_includes_security_fields`** — assert `security_profile`, `shell_enabled`, `github_enabled` present
2. **`activate_project_ro_excludes_security_fields`** — assert those fields absent
3. **`activate_project_includes_memories`** — assert `memories` array present
4. **`activate_project_includes_workspace`** — assert `workspace` array present for multi-project
5. **`activate_project_includes_index_status`** — assert `index` object present
6. **`activate_project_focus_switch_returns_full_response`** — assert focus-switch path returns same shape as full activation (languages, memories, index, workspace all present)
7. **`activate_project_focus_switch_promotes_dormant`** — assert that after focus-switch, `with_project()` succeeds (project is Activated, not Dormant)
8. **`activate_project_rw_hint_promotes_project_status`** — assert hint contains "project_status"
9. **`activate_project_ro_hint_warns_switch_back`** — assert hint contains "remember to activate_project"
10. **`activate_project_auto_libs_summary`** — assert `auto_registered_libs` is `{count, without_source}` not an array
11. **`activate_project_single_project_no_workspace`** — assert `workspace` is null and `format_compact` omits workspace segment
12. **`activate_project_workspace_includes_depends_on`** — assert workspace entries include `depends_on` field
13. **`activate_project_memories_graceful_on_error`** — assert `memories: []` when `MemoryStore` is inaccessible
14. **Update existing `format_activate_project_*` tests** for new compact format with `(rw)`/`(ro)` and workspace count

### Migration

No backward compatibility needed — `activate_project` is consumed by LLMs, not programmatic clients. The new shape is strictly better (less noise, more actionable info). Ship as a clean replacement.
