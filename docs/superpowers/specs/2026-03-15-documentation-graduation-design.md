# Documentation Graduation & Gap-Fill — Design Spec

**Date:** 2026-03-15
**Scope:** Graduate experimental features to stable docs, fill gaps, write agent guides, refresh README
**Approach:** Four layered commits, executed in order (each depends on the previous)

---

## Context

The `experiments` branch was merged to `master` on 2026-03-15 (`e4f0ff1`), bringing three major features:

1. **Multi-project workspace support** — `workspace.toml`, per-project memory/LSP/indexing, cross-project search
2. **Library navigation enhancements** — per-library embedding DBs, version tracking, staleness hints, auto-discovery
3. **LSP idle TTL eviction** — per-language configurable idle timeouts, transparent restart

All three have documentation in `docs/manual/src/experimental/` but have NOT been graduated to the stable manual. Additionally, three agent integration pages are empty stubs, the README still links to experiments, and tool reference pages don't document the `project` workspace parameter.

---

## Layer 1: Graduate Experimental Pages

**Commit message:** `docs: graduate workspace, library nav, LSP idle TTL from experimental to stable`

### Moves

| Source | Target |
|--------|--------|
| `experimental/multi-project-workspace.md` | `concepts/multi-project-workspace.md` |
| `experimental/lsp-idle-ttl.md` | `concepts/lsp-idle-ttl.md` |
| `experimental/library-navigation.md` | merged into `concepts/library-navigation.md` |

### Edits per moved file

- Remove the `> ⚠ Experimental — may change without notice.` callout
- Remove any "on the `experiments` branch" / "available on experiments only" language
- Update relative links (e.g., `../concepts/library-navigation.md` → `library-navigation.md` for same-directory refs)
- Delete the source file from `experimental/` after moving

### Library navigation merge strategy

The stable `concepts/library-navigation.md` (69 lines) covers: auto-discovery, scope parameter, building an index, when to use.

The experimental `experimental/library-navigation.md` (113 lines) adds: per-library embedding DBs, version tracking & staleness hints, auto-discovery details, scope filtering table.

**Merge plan:** Append the experimental content as new sections in the stable file:
- Add `## Per-Library Embedding Databases` after the existing "Building a Library Index" section
- Add `## Version Tracking and Staleness Hints` after that
- Update the "Building a Library Index" section to reference the new per-library DB layout
- Remove the experimental file's intro paragraph and "Further Reading" (redundant in merged form)

### SUMMARY.md changes

```diff
 - [Library Navigation](concepts/library-navigation.md)
+- [Multi-Project Workspaces](concepts/multi-project-workspace.md)
+- [LSP Idle TTL](concepts/lsp-idle-ttl.md)
```

Place after "Library Navigation" and before "Memory".

### experimental/index.md

Replace content with:

```markdown
# Experimental Features

No features are currently in experimental status. All previously experimental
features have graduated to the stable manual.
```

---

## Layer 2: Gap-Fill Existing Docs

**Commit message:** `docs: add workspace project parameter, workspace.toml config, update architecture and roadmap`

### 2a. Tool reference — `project` parameter (4 files)

**`tools/symbol-navigation.md`** — add a subsection after the existing "Scope parameter" section:

```markdown
### Workspace project scoping

In a multi-project workspace, pass `project` to scope operations to a specific
project:

\`\`\`json
{ "tool": "find_symbol", "arguments": { "pattern": "UserService", "project": "backend" } }
\`\`\`

`scope` and `project` are independent axes: `scope` selects project vs library
code, `project` selects which project in the workspace.
```

**`tools/semantic-search.md`** — same pattern, add after scope docs:

```markdown
### Workspace project scoping

\`\`\`json
{ "tool": "semantic_search", "arguments": { "query": "auth flow", "project": "frontend" } }
\`\`\`

Omit `project` to search across the workspace-level context.
```

**`tools/memory.md`** — add a subsection:

```markdown
### Per-project memory

In workspaces, scope memory to a specific project:

\`\`\`json
{ "tool": "memory", "arguments": { "action": "read", "project": "backend", "topic": "architecture" } }
\`\`\`

Omitting `project` reads workspace-level memory.
```

**`tools/workflow-and-config.md`** — add to the `onboarding` section:

```markdown
### Workspace-aware onboarding

For multi-project workspaces, onboarding automatically detects all projects
registered in `workspace.toml` and generates per-project Navigation Strategy
sections in the system prompt. It also writes per-project memories and
cross-project semantic search guidance.
```

### 2b. Configuration — workspace.toml

**`configuration/project-toml.md`** — add a new top-level section at the end:

```markdown
## Workspace Configuration

For multi-project repos, create `.codescout/workspace.toml` alongside
`project.toml`:

\`\`\`toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
depends_on = ["backend"]
\`\`\`

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique project identifier, used in `project` parameter |
| `root` | Yes | Path relative to workspace root |
| `languages` | No | Restrict LSP servers to listed languages |
| `depends_on` | No | Project IDs whose symbols are visible during cross-project navigation |

Each project gets its own LSP servers, memory store, and semantic index.
See [Multi-Project Workspaces](../concepts/multi-project-workspace.md) for
usage details.
```

### 2c. Architecture — `docs/ARCHITECTURE.md`

Add three subsections to the Components section:

**Workspace Registry** (after existing "Library Registry"):
- `src/workspace.rs` — discovers `workspace.toml`, builds project topology, infers `depends_on` from import analysis
- `src/config/workspace.rs` — parses `workspace.toml` into `WorkspaceConfig`
- Each project gets independent `ActiveProject` state in the `Agent`

**Library Version Tracking** (extend existing "Library Registry"):
- `src/library/versions.rs` — reads lockfiles (`Cargo.lock`, `package-lock.json`, etc.) to record indexed vs current versions
- `semantic_search` includes `stale_libraries` when indexed version ≠ current version

**LSP Idle TTL** (extend existing "LSP Client"):
- `src/lsp/manager.rs` — per-language configurable idle timeouts, background eviction task
- Kotlin: 2h (slow startup), all others: 30min
- Transparent restart on next query

### 2d. Roadmap — `docs/ROADMAP.md`

- Move "Multi-project workspace support", "Library navigation enhancements", "LSP idle TTL eviction" to the "What's Built" section
- Update "What's Next" to reflect actual planned work (tree-sitter grammars, additional LSP configs, configurable TTL via project.toml)

---

## Layer 3: Agent Integration Pages

**Commit message:** `docs: update agent integration guides with workspace features`

> **Note:** The agent guides are NOT empty stubs. Real content lives in `docs/agents/`
> (49/207/217 lines for Claude Code/Copilot/Cursor respectively). The mdbook files in
> `docs/manual/src/agents/` are `{{#include}}` directives that pull from there. Layer 3
> audits and updates these existing guides — it does NOT rewrite them from scratch.

### All three guides — audit checklist

For each of `docs/agents/claude-code.md`, `copilot.md`, `cursor.md`:
- Remove any references to the `experiments` branch
- Add a section on **workspace support**: `workspace.toml` registration, `project` parameter, per-project memory
- Add a note about **library navigation**: scope parameter, version staleness hints
- Verify installation instructions are current

### `agents/claude-code.md` — expand (currently 49 lines, target ~120 lines)

This is the shortest guide despite Claude Code being the primary integration. Add:
1. **The routing plugin** — what `code-explorer-routing` does, PreToolUse hooks, why it blocks native Read/Grep/Glob, how to install, how to disable (`block_reads: false`)
2. **Daily workflow** — navigate → understand → edit → verify cycle
3. **Tips** — buffer refs, `run_command` for shell, semantic search for exploration
4. **Workspace support** — workspace.toml, project parameter, workspace-aware onboarding

### `agents/copilot.md` — minor updates (currently 207 lines)

Already comprehensive. Updates:
- Add workspace support subsection
- Remove any experiments branch references
- Verify MCP config format is current

### `agents/cursor.md` — minor updates (currently 217 lines)

Already comprehensive. Updates:
- Add workspace support subsection
- Remove any experiments branch references
- Verify MCP config format is current

### `agents/overview.md` — expand (currently 11 lines, target ~30 lines)

Add:
- One-paragraph intro on MCP server registration
- Feature comparison table: routing plugin, hooks, workspace support per agent
- Links to individual guides

---

## Layer 4: README Refresh

**Commit message:** `docs: refresh README with graduated features and agent guides`

### Structure (~120 lines)

1. **Title + tagline** — keep existing "MCP server giving AI coding agents IDE-grade code intelligence"
2. **What it does** — four bullet pillars:
   - Symbol navigation (LSP, 9 languages)
   - Semantic search (embeddings, concept-level)
   - Library navigation (scope filtering, version tracking, auto-discovery)
   - Multi-project workspaces (per-project memory, LSP, indexing)
3. **Why not just read files?** — keep existing comparison table
4. **Quick start** — keep `cargo install` + MCP config + onboarding
5. **Agent integrations** — table with links to the three guides (already exists, verify links)
6. **Tools (29)** — keep compact category summary
7. **Features** — replace "Experimental Features" section with a stable "Features" section listing: workspace support, library navigation with version tracking, LSP idle TTL, persistent memory, output buffers, progressive disclosure
8. **Contributing + License** — keep as-is

### Key changes from current README
- Remove "Experimental Features" section that links to experiments branch
- Add library navigation and workspace to the "What it does" bullets
- Add "LSP idle TTL" to features list
- Agent guide links already point to `docs/agents/` which is correct (the real content lives there; mdbook includes reference it)

---

## Out of Scope

- Server instructions (`src/prompts/server_instructions.md`) — already current per audit
- Onboarding prompts — already current
- Cargo.toml metadata — already current (v0.3.0)
- Tool misbehaviors doc — maintained separately
- CLAUDE.md — maintained separately

---

## Success Criteria

1. `experimental/index.md` has no features listed; graduated source files deleted from `experimental/`
2. All three features appear in stable manual with no experimental callouts
3. SUMMARY.md links work (no broken links)
4. Agent guides mention workspace features and do not reference the experiments branch
5. Claude Code guide expanded to cover routing plugin and daily workflow (>100 lines)
6. README no longer references experiments branch for features
7. `project` parameter documented in tool reference pages
8. `workspace.toml` documented in configuration
9. ARCHITECTURE.md covers workspace, library versioning, LSP idle TTL
10. ROADMAP.md reflects current state
