# Documentation Graduation & Gap-Fill — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Graduate three experimental features to stable docs, fill documentation gaps, update agent guides, and refresh the README after the experiments→master merge.

**Architecture:** Four sequential commits — graduation (file moves), gap-fill (edits to existing docs), agent guide updates, README refresh. Each layer depends on the previous.

**Tech Stack:** Markdown files only. No code changes.

---

## File Map

### Layer 1: Graduate
- Move: `docs/manual/src/experimental/multi-project-workspace.md` → `docs/manual/src/concepts/multi-project-workspace.md`
- Move: `docs/manual/src/experimental/lsp-idle-ttl.md` → `docs/manual/src/concepts/lsp-idle-ttl.md`
- Merge into: `docs/manual/src/concepts/library-navigation.md` (from `docs/manual/src/experimental/library-navigation.md`)
- Delete: `docs/manual/src/experimental/multi-project-workspace.md`
- Delete: `docs/manual/src/experimental/lsp-idle-ttl.md`
- Delete: `docs/manual/src/experimental/library-navigation.md`
- Modify: `docs/manual/src/experimental/index.md`
- Modify: `docs/manual/src/SUMMARY.md`

### Layer 2: Gap-fill
- Modify: `docs/manual/src/tools/symbol-navigation.md`
- Modify: `docs/manual/src/tools/semantic-search.md`
- Modify: `docs/manual/src/tools/memory.md`
- Modify: `docs/manual/src/tools/workflow-and-config.md`
- Modify: `docs/manual/src/configuration/project-toml.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/ROADMAP.md`

### Layer 3: Agent guides
- Modify: `docs/agents/claude-code.md`
- Modify: `docs/agents/copilot.md`
- Modify: `docs/agents/cursor.md`
- Modify: `docs/manual/src/agents/overview.md`

### Layer 4: README
- Modify: `README.md`

---

## Chunk 1: Layer 1 — Graduate Experimental Pages

### Task 1: Move multi-project-workspace.md to concepts

**Files:**
- Move: `docs/manual/src/experimental/multi-project-workspace.md` → `docs/manual/src/concepts/multi-project-workspace.md`

- [ ] **Step 1: Copy file to new location**

```bash
cp docs/manual/src/experimental/multi-project-workspace.md docs/manual/src/concepts/multi-project-workspace.md
```

- [ ] **Step 2: Remove experimental callout and branch references**

Edit `docs/manual/src/concepts/multi-project-workspace.md`:
- Remove the line `> ⚠ Experimental — may change without notice.`
- Remove/rewrite any "on the `experiments` branch" or "available on experiments only" language
- The file has no relative links to other experimental pages, so no link updates needed

- [ ] **Step 3: Delete source file**

```bash
rm docs/manual/src/experimental/multi-project-workspace.md
```

### Task 2: Move lsp-idle-ttl.md to concepts

**Files:**
- Move: `docs/manual/src/experimental/lsp-idle-ttl.md` → `docs/manual/src/concepts/lsp-idle-ttl.md`

- [ ] **Step 1: Copy file to new location**

```bash
cp docs/manual/src/experimental/lsp-idle-ttl.md docs/manual/src/concepts/lsp-idle-ttl.md
```

- [ ] **Step 2: Remove experimental callout**

Edit `docs/manual/src/concepts/lsp-idle-ttl.md`:
- Remove the line `> ⚠ Experimental — may change without notice.`

- [ ] **Step 3: Delete source file**

```bash
rm docs/manual/src/experimental/lsp-idle-ttl.md
```

### Task 3: Merge library-navigation experimental into stable

**Files:**
- Modify: `docs/manual/src/concepts/library-navigation.md`
- Delete: `docs/manual/src/experimental/library-navigation.md`

The stable file (69 lines) covers: auto-discovery, scope parameter, building an index, when to use.
The experimental file (113 lines) adds: per-library embedding DBs, version tracking, staleness hints, enhanced auto-discovery, scope filtering table.

- [ ] **Step 1: Append new sections to stable file**

After the existing "Building a Library Index" section (which ends with the `.codescout/libraries/<name>/embeddings.db` reference), append these sections from the experimental file:

```markdown
## Per-Library Embedding Databases

Earlier versions stored all embeddings in a single `.codescout/embeddings.db`.
The current layout splits storage into separate databases:

\```
.codescout/
  embeddings/
    project.db          ← your project's code
    lib/
      tokio.db          ← one file per registered library
      serde.db
      reqwest.db
\```

The filename for each library is derived from its registered name: `/` and `\`
are replaced with `--` and the result is lowercased (e.g. `@org/pkg` →
`org--pkg.db`).

**Migration is automatic.** If an old `embeddings.db` is found, codescout moves
its contents into the new structure the first time the project is opened. No
manual steps required.

To build or rebuild a library's index:

\```json
{ "tool": "index_project", "arguments": { "scope": "lib:tokio" } }
\```

## Version Tracking and Staleness Hints

When `index_project(scope="lib:<name>")` runs, codescout reads the project's
lockfile (`Cargo.lock`, `package-lock.json`, etc.) to record the library version
that was indexed.

After a dependency upgrade, `semantic_search` includes a `stale_libraries` field:

\```json
{
  "stale_libraries": [
    {
      "name": "tokio",
      "indexed": "1.37.0",
      "current": "1.38.0",
      "hint": "tokio was updated — run index_project(scope='lib:tokio') to re-index"
    }
  ]
}
\```

Staleness is detected by comparing indexed vs current versions from the lockfile.
If the lockfile ecosystem is not recognised, version tracking is skipped.
```

- [ ] **Step 2: Update "Building a Library Index" section**

Replace the existing paragraph about index storage path to reference the new per-library DB layout. Change:
```
The index persists in `.codescout/libraries/<name>/embeddings.db`.
```
to:
```
The index persists in `.codescout/embeddings/lib/<name>.db` — see
[Per-Library Embedding Databases](#per-library-embedding-databases) below.
```

- [ ] **Step 3: Delete experimental source**

```bash
rm docs/manual/src/experimental/library-navigation.md
```

### Task 4: Update SUMMARY.md

**Files:**
- Modify: `docs/manual/src/SUMMARY.md`

- [ ] **Step 1: Add graduated pages**

Find the line:
```
- [Library Navigation](concepts/library-navigation.md)
```

Add after it:
```
- [Multi-Project Workspaces](concepts/multi-project-workspace.md)
- [LSP Idle TTL](concepts/lsp-idle-ttl.md)
```

These are top-level list items (same indentation as Library Navigation), placed before "Memory".

- [ ] **Step 2: Verify no experimental links in SUMMARY.md**

Search SUMMARY.md for any remaining links to `experimental/`. There should be none (the experimental index was never in SUMMARY.md).

### Task 5: Update experimental/index.md

**Files:**
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Replace entire content**

Replace the full file content with:

```markdown
# Experimental Features

No features are currently in experimental status. All previously experimental
features have graduated to the stable manual.
```

### Task 6: Commit Layer 1

- [ ] **Step 1: Stage and commit**

```bash
git add docs/manual/src/concepts/multi-project-workspace.md \
       docs/manual/src/concepts/lsp-idle-ttl.md \
       docs/manual/src/concepts/library-navigation.md \
       docs/manual/src/experimental/ \
       docs/manual/src/SUMMARY.md
git commit -m "docs: graduate workspace, library nav, LSP idle TTL from experimental to stable"
```

---

## Chunk 2: Layer 2 — Gap-Fill Existing Docs

### Task 7: Add `project` parameter to symbol-navigation.md

**Files:**
- Modify: `docs/manual/src/tools/symbol-navigation.md`

- [ ] **Step 1: Add workspace scoping subsection**

Find the paragraph that starts with "**Scope parameter:**" (around line 14). After that paragraph, add:

```markdown
### Workspace project scoping

In a [multi-project workspace](../concepts/multi-project-workspace.md), pass
`project` to scope operations to a specific project:

\```json
{ "tool": "find_symbol", "arguments": { "pattern": "UserService", "project": "backend" } }
\```

`scope` and `project` are independent axes: `scope` selects project vs library
code, `project` selects which project in the workspace. Omitting `project`
uses the workspace-level context.
```

### Task 8: Add `project` parameter to semantic-search.md

**Files:**
- Modify: `docs/manual/src/tools/semantic-search.md`

- [ ] **Step 1: Add workspace scoping subsection**

After the existing scope documentation, add:

```markdown
### Workspace project scoping

\```json
{ "tool": "semantic_search", "arguments": { "query": "auth flow", "project": "frontend" } }
\```

Omit `project` to search across the workspace-level context. See
[Multi-Project Workspaces](../concepts/multi-project-workspace.md) for setup.
```

### Task 9: Add `project` parameter to memory.md

**Files:**
- Modify: `docs/manual/src/tools/memory.md`

- [ ] **Step 1: Add per-project memory subsection**

After the existing action documentation, add:

```markdown
### Per-project memory

In [workspaces](../concepts/multi-project-workspace.md), scope memory to a
specific project:

\```json
{ "tool": "memory", "arguments": { "action": "read", "project": "backend", "topic": "architecture" } }
\```

Omitting `project` reads/writes workspace-level memory.
```

### Task 10: Add workspace-aware onboarding to workflow-and-config.md

**Files:**
- Modify: `docs/manual/src/tools/workflow-and-config.md`

- [ ] **Step 1: Add workspace onboarding subsection**

After the existing `onboarding` tool documentation, add:

```markdown
### Workspace-aware onboarding

For [multi-project workspaces](../concepts/multi-project-workspace.md),
onboarding automatically detects all projects registered in `workspace.toml`
and generates per-project Navigation Strategy sections in the system prompt.
It also writes per-project memories and cross-project semantic search guidance.

Each project gets its own onboarding pass with language detection, entry point
discovery, and memory writing scoped to that project.
```

### Task 11: Add workspace.toml to configuration docs

**Files:**
- Modify: `docs/manual/src/configuration/project-toml.md`

- [ ] **Step 1: Add workspace configuration section at end of file**

Append:

```markdown
## Workspace Configuration

For multi-project repos, create `.codescout/workspace.toml` alongside
`project.toml`:

\```toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
depends_on = ["backend"]
\```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique project identifier, used in `project` parameter across tools |
| `root` | Yes | Path relative to workspace root |
| `languages` | No | Restrict LSP servers to listed languages |
| `depends_on` | No | Project IDs whose symbols are visible during cross-project navigation |

Each project gets its own LSP servers, memory store, and semantic index.
See [Multi-Project Workspaces](../concepts/multi-project-workspace.md) for
usage details.
```

### Task 12: Update ARCHITECTURE.md

**Files:**
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: Add workspace, library versioning, and LSP idle TTL subsections**

In the Components section, after the existing "Library Registry" subsection (around line 93), add:

```markdown
### Workspace Registry

- `src/workspace.rs` — discovers `.codescout/workspace.toml`, builds project
  topology, resolves `depends_on` relationships between projects
- `src/config/workspace.rs` — parses `workspace.toml` into `WorkspaceConfig`
  with `[[project]]` table entries
- Each project gets independent `ActiveProject` state in the `Agent`, with its
  own LSP servers, memory store, and semantic index
```

Extend the existing "Library Registry" subsection with:

```markdown
- `src/library/versions.rs` — reads lockfiles (`Cargo.lock`, `package-lock.json`,
  etc.) to record indexed vs current versions; `semantic_search` includes
  `stale_libraries` hints when versions diverge
- Per-library embedding databases in `.codescout/embeddings/lib/<name>.db`
```

Extend the existing "LSP Client" subsection with:

```markdown
- Idle TTL eviction (`src/lsp/manager.rs`) — per-language configurable timeouts
  (Kotlin: 2h, all others: 30min). Idle servers are shut down and transparently
  restarted on next query.
```

### Task 13: Update ROADMAP.md

**Files:**
- Modify: `docs/ROADMAP.md`

- [ ] **Step 1: Add graduated features to "What's Built"**

After the existing bullet list in "What's Built" (around line 43), add these bullets:

```markdown
- **Multi-project workspaces** — `workspace.toml` registration, per-project memory/LSP/indexing, cross-project search guidance, workspace-aware onboarding
- **Library version tracking** — per-library embedding DBs (`.codescout/embeddings/lib/`), lockfile version comparison, staleness hints in `semantic_search`
- **LSP idle TTL eviction** — per-language configurable timeouts (Kotlin 2h, others 30min), transparent shutdown and restart
```

- [ ] **Step 2: Update "What's Next"**

Replace the current "What's Next" content:
```markdown
## What's Next

- Additional tree-sitter grammars (currently: Rust, Python, TypeScript, Go, Java, Kotlin)
- Additional LSP server configurations
- Configurable LSP idle TTL via `project.toml`
- GitHub tools: `github_issue`, `github_pr` method parity with `github_repo`
```

### Task 14: Commit Layer 2

- [ ] **Step 1: Stage and commit**

```bash
git add docs/manual/src/tools/symbol-navigation.md \
       docs/manual/src/tools/semantic-search.md \
       docs/manual/src/tools/memory.md \
       docs/manual/src/tools/workflow-and-config.md \
       docs/manual/src/configuration/project-toml.md \
       docs/ARCHITECTURE.md \
       docs/ROADMAP.md
git commit -m "docs: add workspace project parameter, workspace.toml config, update architecture and roadmap"
```

---

## Chunk 3: Layer 3 — Agent Guide Updates

### Task 15: Expand claude-code.md

**Files:**
- Modify: `docs/agents/claude-code.md`

Currently 49 lines. Target ~120 lines. The existing content covers: setup, workflow skills, routing plugin basics, verify, day-to-day links. Missing: detailed routing plugin behavior, buffer refs, workspace support, tips.

- [ ] **Step 1: Expand the Routing Plugin section**

Replace the brief routing plugin paragraph with a fuller section:

```markdown
## Routing Plugin (codescout-companion)

The routing plugin is a Claude Code plugin that **enforces** codescout tool use via
`PreToolUse` hooks. Without it, the agent may fall back to native `Read`, `Grep`, and
`Glob` tools — which work but bypass codescout's token-efficient symbol navigation.

**What it blocks:**
- `Read` on source files (`.rs`, `.ts`, `.py`, etc.) → redirects to `list_symbols` / `find_symbol`
- `Grep` / `Glob` on source files → redirects to `search_pattern` / `find_file`
- `Bash` for shell commands → redirects to `run_command`

**What it allows:**
- `Read` on non-source files (markdown, TOML, JSON, config)
- All codescout MCP tools pass through unrestricted

Install via:

\```
claude plugin install codescout-companion
\```

Or follow the [Routing Plugin guide](../manual/src/getting-started/routing-plugin.md)
for manual setup.

**Debugging:** If the plugin blocks a legitimate operation, create
`.claude/code-explorer-routing.json` with `{"block_reads": false}` to temporarily
disable blocking.
```

- [ ] **Step 2: Add workspace support section**

After the Verify section, add:

```markdown
## Multi-Project Workspaces

codescout supports multi-project workspaces. Register projects in
`.codescout/workspace.toml`:

\```toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
\```

After onboarding, use the `project` parameter to scope tool calls:

\```
find_symbol("UserService", project: "backend")
memory(action: "read", project: "frontend", topic: "architecture")
\```

See [Multi-Project Workspaces](../manual/src/concepts/multi-project-workspace.md).
```

- [ ] **Step 3: Add tips section**

After the Day-to-Day Workflow section, add:

```markdown
## Tips

**Buffer refs** — When `read_file` or `run_command` returns a `@file_*` or `@cmd_*`
handle, the content is stored server-side. Query it with
`run_command("grep pattern @cmd_xxxx")` or read sub-ranges with
`read_file("@file_xxxx", start_line=1, end_line=100)`.

**Semantic search for exploration** — When entering an unfamiliar part of the codebase,
start with `semantic_search("how does X work")` rather than reading files. It returns
ranked code chunks by relevance.

**Memory for cross-session context** — Use `memory(action: "remember", content: "...")`
to store decisions, patterns, or gotchas. Use `memory(action: "recall", query: "...")`
to retrieve them by meaning in future sessions.

**Library navigation** — When `goto_definition` resolves to a dependency, codescout
auto-registers the library. Use `semantic_search(scope: "lib:tokio")` to search
within it.
```

### Task 16: Update copilot.md

**Files:**
- Modify: `docs/agents/copilot.md`

- [ ] **Step 1: Add workspace support section**

After the existing "Enforcement Hook" or "Workflow" section, add:

```markdown
## Multi-Project Workspaces

codescout supports multi-project workspaces via `.codescout/workspace.toml`.
After onboarding, pass `project` to scope tool calls to a specific project:

\```json
{ "tool": "find_symbol", "arguments": { "pattern": "UserService", "project": "backend" } }
\```

See [Multi-Project Workspaces](../manual/src/concepts/multi-project-workspace.md).
```

- [ ] **Step 2: Audit for experiments branch references**

Search for "experiments" or "experimental" in the file. Remove or update any references.

### Task 17: Update cursor.md

**Files:**
- Modify: `docs/agents/cursor.md`

- [ ] **Step 1: Add workspace support section**

Same content as Task 16 Step 1 — add the workspace section after the workflow section.

- [ ] **Step 2: Audit for experiments branch references**

Same as Task 16 Step 2.

### Task 18: Expand agents/overview.md

**Files:**
- Modify: `docs/manual/src/agents/overview.md`

- [ ] **Step 1: Replace entire content**

```markdown
# Agent Integrations

codescout works with any MCP-capable coding agent. Once registered as an MCP
server, codescout's system prompt injects automatically into every session,
giving the agent tool selection rules and iron laws for code navigation.

## Feature comparison

| Feature | Claude Code | GitHub Copilot | Cursor |
|---------|-------------|----------------|--------|
| MCP protocol | stdio | stdio | stdio |
| System prompt injection | Automatic | Automatic | Automatic |
| Tool enforcement (routing plugin) | Plugin with hooks | Copilot Skill guidance | Cursor Rules guidance |
| Workspace support | Full | Full | Full |
| Onboarding | Automatic | Automatic | Automatic |

## Guides

| Agent | Guide |
|---|---|
| Claude Code | [Claude Code](claude-code.md) — primary integration with routing plugin enforcement |
| GitHub Copilot | [GitHub Copilot](copilot.md) — VS Code extension with Skills-based guidance |
| Cursor | [Cursor](cursor.md) — Cursor Rules-based guidance |
```

### Task 19: Commit Layer 3

- [ ] **Step 1: Stage and commit**

```bash
git add docs/agents/claude-code.md \
       docs/agents/copilot.md \
       docs/agents/cursor.md \
       docs/manual/src/agents/overview.md
git commit -m "docs: update agent integration guides with workspace features"
```

---

## Chunk 4: Layer 4 — README Refresh

### Task 20: Refresh README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update "What it does" section**

Replace the current three-bullet list with four bullets:

```markdown
## What it does

- **Symbol navigation** — `find_symbol`, `list_symbols`, `find_references`, `goto_definition`, `replace_symbol`, backed by LSP across 9 languages
- **Semantic search** — find code by concept using embeddings, not grep
- **Library navigation** — explore dependency source code with scoped search, version tracking, and auto-discovery
- **Multi-project workspaces** — register related projects in `workspace.toml` for cross-project navigation with per-project memory and indexing
- **Token efficiency** — compact by default, details on demand, never dumps full files
```

- [ ] **Step 2: Replace "Experimental Features" section with "Features"**

Replace the section:
```markdown
## Experimental Features

New features land on the `experiments` branch before reaching `master`.
They may change or be removed without notice, and may not be in your installed release yet.

→ [Browse experimental features](https://github.com/mareurs/codescout/blob/experiments/docs/manual/src/experimental/index.md)
```

With:
```markdown
## Features

- Multi-project workspace support with per-project LSP, memory, and semantic indexing
- Library navigation with per-library embedding databases and version staleness hints
- LSP idle TTL — idle language servers are shut down automatically (Kotlin: 2h, others: 30min) and restarted transparently on next query
- Persistent memory across sessions with semantic recall
- Output buffers (`@cmd_*`, `@file_*`) for token-efficient large output handling
- Progressive disclosure — compact by default, full detail on demand
```

- [ ] **Step 3: Verify agent guide links**

Confirm the existing agent guide table links point to `docs/agents/claude-code.md` etc. These are correct — the real content lives at `docs/agents/`.

### Task 21: Commit Layer 4

- [ ] **Step 1: Stage and commit**

```bash
git add README.md
git commit -m "docs: refresh README with graduated features and agent guides"
```

---

## Verification

After all four commits:

- [ ] All links in SUMMARY.md resolve (no broken links)
- [ ] `experimental/` contains only `index.md` with the "no features" message
- [ ] `git grep "Experimental.*may change"` returns 0 hits outside `experimental/index.md`
- [ ] `git grep "experiments branch" docs/` returns 0 hits
- [ ] README does not reference experiments branch
- [ ] Agent guides mention workspace features
- [ ] Claude Code guide is >100 lines
