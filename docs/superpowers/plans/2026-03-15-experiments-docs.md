# Experiments Documentation Workflow Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the experiments-branch documentation workflow into the project — create the staging area, update CLAUDE.md rules, update README, and backfill existing features.

**Architecture:** Pure documentation and configuration changes — no code. The staging area is `docs/manual/src/experimental/` (not wired into SUMMARY.md). CLAUDE.md gets two new rule sections. README gets a short Experimental Features entry. Three existing features on `experiments` get backfill pages. The `index.md` starts empty and is updated as each backfill page is added.

**Tech Stack:** Markdown, mdBook (for the manual), git

**Spec:** `docs/superpowers/specs/2026-03-15-experiments-docs-design.md`

---

## Chunk 1: Infrastructure + Rules

### Task 1: Create the experimental staging area (empty state)

**Files:**
- Create: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create the directory and `index.md` with empty feature list**

Create `docs/manual/src/experimental/index.md` with this exact content:

```markdown
# Experimental Features

> ⚠ **These features are on the `experiments` branch and may change or be removed without
> notice. They may not be present in your installed release.**

Features listed here are working but not yet merged to `master`. To try them:

```bash
git clone https://github.com/mareurs/codescout
cd codescout
git checkout experiments
cargo build --release
```

Then register the locally built binary in your MCP config instead of the installed `codescout`.

## Features in development

No experimental features at this time.
```

- [ ] **Step 2: Verify the file exists and has the empty-state line**

Run: `grep "No experimental" docs/manual/src/experimental/index.md`
Expected: one match — `No experimental features at this time.`

- [ ] **Step 3: Commit**

```bash
git add docs/manual/src/experimental/index.md
git commit -m "docs(experimental): create experimental staging area with empty index"
```

---

### Task 2: Add documentation rules to CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` — add two sections between `### Branch Strategy` and `### Publishing to crates.io`

- [ ] **Step 1: Locate the insertion point**

Open `CLAUDE.md`. Find `### Publishing to crates.io`. The two new sections go **directly before it**, immediately after the closing bullet of `### Branch Strategy`.

- [ ] **Step 2: Insert the two new sections**

Add the following text between the end of `### Branch Strategy` and the start of `### Publishing to crates.io`:

```markdown
### Documenting Features on `experiments`

When adding a feature commit to `experiments`, you MUST include documentation in the same commit:

1. Create `docs/manual/src/experimental/<feature-name>.md` — written as final user-facing
   docs with a single `> ⚠ Experimental — may change without notice.` callout at the top.
2. Add a line to `docs/manual/src/experimental/index.md` linking to the new page.

**Only features, not bug fixes.** Bug fixes need no experimental doc.

**If a feature is removed from `experiments`** (reverted or abandoned), delete its page and
remove its entry from `index.md` in the same commit.

### Graduating a Feature (`experiments` → `master`)

When cherry-picking a feature to `master`, use `--no-commit` to bundle the doc graduation
into the same commit:

```bash
git cherry-pick --no-commit <sha>
# then make the four graduation changes:
# 1. Move docs/manual/src/experimental/<feature-name>.md to its target chapter
# 2. Remove the `> ⚠ Experimental` callout from the top of the page
# 3. Add the page to docs/manual/src/SUMMARY.md in the right place
# 4. Remove the feature's entry from docs/manual/src/experimental/index.md
git commit -m "feat(...): <description>"
```

**Rebase note:** Because the graduation commit on `master` includes additional doc changes,
its patch differs from the original `experiments` commit. Git will **not** auto-skip it
during the subsequent `git rebase master` on `experiments`. After rebasing, drop the
now-superseded original commit manually:

```bash
git checkout experiments
git rebase master          # the original feature commit will NOT be auto-dropped
git rebase -i master       # drop the original feature commit from the list
```
```

- [ ] **Step 3: Verify placement — sections appear after Branch Strategy, before Publishing**

Run: `grep -n "Branch Strategy\|Documenting Features\|Graduating a Feature\|Publishing to crates" CLAUDE.md`

Expected output (line numbers will differ, but order must be):
```
N:### Branch Strategy
M:### Documenting Features on `experiments`
P:### Graduating a Feature (`experiments` → `master`)
Q:### Publishing to crates.io
```
where N < M < P < Q.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): add experiments documentation and graduation rules"
```

---

### Task 3: Add Experimental Features section to README

**Files:**
- Modify: `README.md` — add section after `## Contributing`

- [ ] **Step 1: Add the section after `## Contributing`**

Open `README.md`. Find `## Contributing`. Add the following immediately after the Contributing section (before `## License`):

```markdown
## Experimental Features

New features land on the `experiments` branch before reaching `master`.
They may change or be removed without notice, and may not be in your installed release yet.

→ [Browse experimental features](https://github.com/mareurs/codescout/blob/experiments/docs/manual/src/experimental/index.md)
```

- [ ] **Step 2: Verify correct placement**

Run: `grep -n "Contributing\|Experimental Features\|License" README.md`

Expected output — Contributing line number < Experimental Features line number < License line number:
```
N:## Contributing
M:## Experimental Features
P:## License
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): add Experimental Features section linking to experiments branch"
```

---

## Chunk 2: Backfill Existing Features

> These three pages document features already on `experiments` that predate this workflow.
> Each page follows the same structure: `> ⚠ Experimental` callout, then full user-facing docs.
> Each task also updates `index.md` to add its feature entry (replacing the empty-state line
> after the first task, appending after the last entry for subsequent tasks).

### Task 4: Backfill — LSP Idle TTL Eviction

**Files:**
- Create: `docs/manual/src/experimental/lsp-idle-ttl.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create the feature page**

Create `docs/manual/src/experimental/lsp-idle-ttl.md`:

```markdown
# LSP Idle TTL Eviction

> ⚠ Experimental — may change without notice.

codescout starts LSP servers on demand and keeps them running for fast symbol lookups.
A server that has been idle beyond its timeout is shut down automatically to reclaim memory.

## Default timeouts

| Language | Idle TTL |
|---|---|
| Kotlin | 2 hours |
| All others | 30 minutes |

Kotlin gets a longer TTL because its LSP server has a long startup time — evicting it
aggressively would cause noticeable latency on the next query.

## Behaviour

When an LSP server's idle TTL expires:

1. codescout sends a `shutdown` request and `exit` notification to the server.
2. The server process is removed from the pool.
3. On the next symbol request for that language, a new server is started automatically.

There is no user-visible interruption — the eviction and restart are transparent.

## Configuration

TTL eviction is not yet configurable via `project.toml`. This is planned for a future release.
```

- [ ] **Step 2: Update `index.md` — replace empty-state line with first feature entry**

In `docs/manual/src/experimental/index.md`, replace:

```
No experimental features at this time.
```

with:

```
- [LSP Idle TTL Eviction](lsp-idle-ttl.md) — automatically shut down idle LSP servers to
  reclaim memory, with per-language configurable timeouts
```

- [ ] **Step 3: Verify both files**

Run: `ls docs/manual/src/experimental/ && grep "lsp-idle-ttl" docs/manual/src/experimental/index.md`
Expected: directory listing includes `lsp-idle-ttl.md`; grep finds the link.

- [ ] **Step 4: Commit**

```bash
git add docs/manual/src/experimental/lsp-idle-ttl.md docs/manual/src/experimental/index.md
git commit -m "docs(experimental): backfill LSP idle TTL eviction"
```

---

### Task 5: Backfill — Multi-Project Workspace Support

**Files:**
- Create: `docs/manual/src/experimental/multi-project-workspace.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create the feature page**

Create `docs/manual/src/experimental/multi-project-workspace.md`:

```markdown
# Multi-Project Workspace Support

> ⚠ Experimental — may change without notice.

codescout can manage multiple related projects from a single server instance.
This is useful for monorepos or closely related repositories where you want
cross-project navigation without running separate MCP servers.

## Registering projects

Projects are registered in `.codescout/project.toml` under a `[[workspace.projects]]` table:

```toml
[[workspace.projects]]
id = "backend"
root = "services/backend"

[[workspace.projects]]
id = "frontend"
root = "apps/frontend"
```

Each project gets its own LSP servers, memory store, and semantic index.

## Using project scope

Most tools accept a `project` parameter to scope the operation:

```
find_symbol("MyStruct", project: "backend")
semantic_search("authentication flow", project: "frontend")
memory(action: "read", project: "backend", topic: "architecture")
```

Omitting `project` uses the workspace-level context.

## Onboarding

Run onboarding once after registering projects:

```
Run codescout onboarding
```

codescout generates a per-project Navigation Strategy section in the system prompt so the
agent knows which files and entry points belong to each project. It also generates
cross-project semantic search scope guidance.

## Cross-project semantic search

The system prompt includes guidance on which `scope=` values to use for semantic search
across projects, so the agent does not need to guess project boundaries.
```

- [ ] **Step 2: Update `index.md` — append the feature entry**

In `docs/manual/src/experimental/index.md`, after the `lsp-idle-ttl` entry, add:

```
- [Multi-Project Workspace Support](multi-project-workspace.md) — register and navigate
  multiple related projects from a single codescout instance
```

- [ ] **Step 3: Verify both files**

Run: `grep "multi-project-workspace\|lsp-idle-ttl" docs/manual/src/experimental/index.md`
Expected: two matching lines.

- [ ] **Step 4: Commit**

```bash
git add docs/manual/src/experimental/multi-project-workspace.md docs/manual/src/experimental/index.md
git commit -m "docs(experimental): backfill multi-project workspace support"
```

---

### Task 6: Backfill — Library Navigation Enhancements

**Files:**
- Create: `docs/manual/src/experimental/library-navigation.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create the feature page**

Create `docs/manual/src/experimental/library-navigation.md`:

```markdown
# Library Navigation Enhancements

> ⚠ Experimental — may change without notice.

This page documents enhancements to library navigation beyond what is in the current
`master` release. See [Library Navigation](../concepts/library-navigation.md) for the
stable baseline.

## Per-library embedding databases

Each registered library now gets its own embedding database:
`.codescout/embeddings-<name>.db`. This means:

- Indexing one library does not invalidate another library's index.
- Libraries can be re-indexed independently: `index_project(scope="lib:<name>")`

## Version tracking and staleness hints

codescout tracks the version of each library that was indexed (from its lockfile or manifest).
When the lockfile version differs from the indexed version, `semantic_search` and
`list_libraries` include a staleness hint prompting you to re-index.

## Auto-discovery

`goto_definition` and `hover` now auto-register libraries when they resolve to a path
outside the project root. You no longer need to run `register_library` manually for
dependencies your LSP server already knows about.

## Scope filtering in symbol tools

`find_symbol` and `semantic_search` accept `scope="lib:<name>"` to search library code:

```
find_symbol("HashMap", scope="lib:std")
semantic_search("connection pooling", scope="lib:sqlx")
```

The library must be registered (and indexed for semantic search) before scope filtering works.
```

- [ ] **Step 2: Update `index.md` — append the feature entry**

In `docs/manual/src/experimental/index.md`, after the `multi-project-workspace` entry, add:

```
- [Library Navigation Enhancements](library-navigation.md) — per-library embedding DBs,
  version tracking, and `scope=` filtering across symbol and semantic tools
```

- [ ] **Step 3: Verify both files**

Run: `grep "library-navigation\|multi-project\|lsp-idle" docs/manual/src/experimental/index.md`
Expected: three matching lines.

- [ ] **Step 4: Commit**

```bash
git add docs/manual/src/experimental/library-navigation.md docs/manual/src/experimental/index.md
git commit -m "docs(experimental): backfill library navigation enhancements"
```

---

### Task 7: Final verification

- [ ] **Step 1: Verify all experimental pages exist**

Run: `ls docs/manual/src/experimental/`
Expected: `index.md`, `lsp-idle-ttl.md`, `multi-project-workspace.md`, `library-navigation.md`

- [ ] **Step 2: Verify index.md has three feature links**

Run: `grep -c "^\- \[" docs/manual/src/experimental/index.md`
Expected: `3`

- [ ] **Step 3: Verify CLAUDE.md sections are in the right order**

Run: `grep -n "Branch Strategy\|Documenting Features\|Graduating a Feature\|Publishing to crates" CLAUDE.md`
Expected: line numbers in ascending order: Branch Strategy < Documenting Features < Graduating a Feature < Publishing to crates

- [ ] **Step 4: Verify README section ordering**

Run: `grep -n "Contributing\|Experimental Features\|License" README.md`
Expected: line numbers in ascending order: Contributing < Experimental Features < License

- [ ] **Step 5: Verify no experimental pages were accidentally added to SUMMARY.md**

Run: `grep "experimental" docs/manual/src/SUMMARY.md`
Expected: no output
