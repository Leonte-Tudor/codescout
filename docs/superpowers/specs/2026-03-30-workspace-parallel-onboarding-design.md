# Workspace Parallel Onboarding

**Date:** 2026-03-30
**Status:** Draft
**Scope:** `src/tools/workflow.rs` (Onboarding), `src/prompts/workspace_onboarding_prompt.md`
**Builds on:** `2026-03-30-onboarding-markdown-navigation-design.md`

## Problem

Workspace onboarding with N projects runs in a single subagent that sequentially
explores every project, writes per-project memories, then synthesizes workspace
memories. For a 4-project workspace this hits 125+ tool calls and 144k tokens,
exhausting the subagent's context window. Projects explored later get less
thorough treatment as context fills up.

## Design

### Core Idea

The server builds N+1 tailored prompt files — one per project plus one synthesis
prompt. The main agent dispatches per-project subagents in parallel, waits for
all to complete, then runs the synthesis step.

### Flow

```
Main Agent
  ├─ calls onboarding() → gets workspace-dispatch response
  ├─ reads Phase 0-1 from main prompt (embedding model + index check)
  ├─ spawns N subagents in parallel (one per project prompt file)
  │   ├─ Subagent 1: read_markdown("...onboarding-project-backend-kotlin.md")
  │   ├─ Subagent 2: read_markdown("...onboarding-project-eduplanner-mcp.md")
  │   ├─ Subagent 3: read_markdown("...onboarding-project-python-services.md")
  │   └─ Subagent 4: read_markdown("...onboarding-project-mcp-deprecated.md")
  ├─ waits for all subagents to complete
  ├─ reads synthesis prompt: read_markdown("...onboarding-workspace-synthesis.md")
  ├─ reads back all per-project memories
  └─ writes workspace-level memories + system prompt
```

### Server Response Shape

When `onboarding()` detects a workspace (>1 project), `call_content()` returns:

```json
{
  "prompt_path": ".codescout/tmp/onboarding-prompt.md",
  "summary": "[kotlin, rust, python, typescript] · workspace (4 projects)",
  "sections": ["1. ## THE IRON LAW (27 lines)", "..."],
  "project_prompts": [
    {
      "id": "backend-kotlin",
      "path": ".codescout/tmp/onboarding-project-backend-kotlin.md",
      "languages": ["kotlin", "java"],
      "root": "."
    },
    {
      "id": "eduplanner-mcp",
      "path": ".codescout/tmp/onboarding-project-eduplanner-mcp.md",
      "languages": ["rust"],
      "root": "eduplanner-mcp/"
    },
    {
      "id": "python-services",
      "path": ".codescout/tmp/onboarding-project-python-services.md",
      "languages": ["python"],
      "root": "python-services/"
    }
  ],
  "synthesis_prompt_path": ".codescout/tmp/onboarding-workspace-synthesis.md",
  "instructions": "... workspace-aware dispatch instructions ..."
}
```

For single-project repos, the response is unchanged (no `project_prompts` or
`synthesis_prompt_path` fields).

### Per-Project Prompt File Contents

Each `.codescout/tmp/onboarding-project-{id}.md` is self-contained (~15-20KB):

```markdown
## THE IRON LAW
[copied from main prompt]

## Your Project
- **ID:** backend-kotlin
- **Root:** . (workspace root)
- **Languages:** kotlin, java
- **Manifest:** build.gradle.kts
- **Sibling projects:** eduplanner-mcp (rust), python-services (python)
  Do NOT deep-dive siblings — they have their own subagents.

## Phase 2: Explore the Code
[full exploration steps from main prompt, scoped to this project's root]

## Red Flags — STOP and Return to Phase 2
[copied from main prompt]

## Common Rationalizations
[copied from main prompt]

## Phase 3: Write the Memories
[6 memory templates, with project= parameter for per-project memory storage]

## Gathered Project Data
[README, build config, entry points, test dirs — for THIS project only]

## Return Contract
[what to return when done]
```

### Synthesis Prompt File Contents

`.codescout/tmp/onboarding-workspace-synthesis.md` contains:

```markdown
## Workspace Memory Synthesis

Read back all per-project memories:
  memory(action="read", topic="project-overview", project="backend-kotlin")
  memory(action="read", topic="architecture", project="backend-kotlin")
  ...repeat for each project...

Then write workspace-level memories:
1. `architecture` — cross-project structure, shared infrastructure, dependency graph
2. `conventions` — shared patterns, naming, testing approaches across projects
3. `development-commands` — workspace-level build/test/lint commands
4. `domain-glossary` — terms that span multiple projects
5. `gotchas` — cross-project pitfalls, version mismatches, integration gotchas

## System Prompt Generation
[template for workspace system prompt with per-project navigation hints]
```

### Workspace-Aware Instructions

**Claude Code (subagent-capable):**

```
Onboarding required — this is a workspace with N projects.

Steps:
1. Read Phase 0-1 from the main prompt:
   read_markdown(".codescout/tmp/onboarding-prompt.md", headings=["## Phase 0: ...", "## Phase 1: ..."])

2. Spawn N subagents IN PARALLEL — one per project:
   - Subagent for backend-kotlin: read_markdown(".codescout/tmp/onboarding-project-backend-kotlin.md")
   - Subagent for eduplanner-mcp: read_markdown(".codescout/tmp/onboarding-project-eduplanner-mcp.md")
   - ...

3. Wait for all subagents to complete.

4. Read the synthesis prompt and write workspace memories:
   read_markdown(".codescout/tmp/onboarding-workspace-synthesis.md")
```

**Other clients (single-agent):**

Same structure but sequential — explore each project one at a time, then
synthesize. Uses the same per-project prompt files.

### File Lifecycle

- All files written to `.codescout/tmp/` (gitignored)
- Overwritten on each `onboarding(force: true)` call
- No cleanup needed — files are small and ephemeral

### What Changes

| Component | Before | After |
|---|---|---|
| Workspace onboarding | 1 subagent, 1 giant prompt | N+1 subagents, N+1 focused prompts |
| Per-project context | ~60KB shared prompt | ~15-20KB tailored prompt per project |
| Token usage per subagent | 144k+ (hits ceiling) | ~30-50k (comfortable) |
| Parallelism | Sequential | Parallel per-project |
| Synthesis | Same subagent, end of context | Fresh main agent, reads memories |

### What Does NOT Change

- Single-project onboarding — unchanged
- `call()` return value — unchanged (still has `subagent_prompt` for programmatic use)
- Phase 0-1 (embedding model, index check) — handled by main agent before dispatch
- Per-project memory storage — already uses `project` parameter
- `onboarding(force: true)` trigger — unchanged

### Server-Side Implementation

The server already has:
- `gather_project_context()` which discovers all projects with languages, manifests
- `build_subagent_preamble()` and `build_subagent_epilogue()`
- Per-project gathered data in `DiscoveredProject`

New code needed:
- `build_per_project_prompt(project, siblings, common_sections)` — assembles the per-project .md file
- `build_synthesis_prompt(projects)` — assembles the workspace synthesis .md file
- Updated `call_content()` — detects workspace, writes N+1 files, returns the dispatch response

## Testing

1. **Unit test**: Single-project onboarding returns no `project_prompts` field
2. **Unit test**: Workspace onboarding returns `project_prompts` array with one entry per project
3. **Unit test**: Per-project prompt files exist on disk after call
4. **Unit test**: Per-project prompt contains project-specific data (root, languages) and NOT sibling project exploration instructions
5. **Unit test**: Synthesis prompt contains memory read-back commands for all projects
6. **Unit test**: Workspace instructions mention parallel subagent dispatch (Claude) or sequential reading (other clients)
7. **Integration test**: Full workspace onboarding flow with 2+ projects, verify per-project memories written
