# Onboarding Subagent Delegation

**Date:** 2026-03-26
**Status:** Draft
**Component:** `src/tools/workflow.rs` (Onboarding tool)

## Problem

The onboarding workflow instructs the main agent to perform 7 deep exploration steps
(full symbol surveys, reading 5+ function bodies, tracing 2 data flows, 5+ concept
queries, reading tests) followed by writing 6-7 structured memories — all inline in
the main conversation context.

For large projects, even single-language ones, this consumes 50-80% of the main
agent's context window before any user task begins. The agent hits context compression
before it starts real work, losing the very exploration context it just built.

## Solution

The `onboarding()` MCP tool returns a **split response**: a self-contained
`subagent_prompt` (opaque blob the main agent passes through) and short
`main_agent_instructions` (~200 tokens) telling the main agent to dispatch a
Sonnet-class subagent. The subagent performs all exploration and memory writing in
an isolated context, then returns a concise summary.

### Why Sonnet (not Haiku)

Onboarding is a one-time setup that every future session depends on. The exploration
requires judgment (picking meaningful data flows, identifying core abstractions) and
the memory writing requires synthesis (capturing *why*, not just *what*). Haiku tends
toward shallow descriptions and is more susceptible to cutting corners on multi-step
instructions. The compound cost of mediocre memories across dozens of sessions
outweighs the one-time savings of a cheaper model.

## Design

### Response Shape Change

When onboarding needs to run (not already onboarded, or `force=true`):

```json
{
  "onboarded": false,
  "languages": ["rust", "python"],
  "config_created": true,
  "has_readme": true,
  "build_file": "Cargo.toml",
  "subagent_prompt": "<full exploration + memory-writing instructions>",
  "main_agent_instructions": "<short dispatch command, ~200 tokens>"
}
```

When already onboarded (fast path): **no change** to the current response shape.

### Main Agent Instructions

```
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and
memory writing. Pass the content of the `subagent_prompt` field as the subagent's
task prompt. Do NOT read or summarize `subagent_prompt` yourself — pass it through
as-is.

The subagent will:
1. Explore the codebase thoroughly (symbol surveys, code reading, data flow tracing)
2. Write project memories (architecture, conventions, gotchas, etc.)
3. Return an exploration summary and list of memories written

When the subagent completes, report its summary to the user. Then read whichever
memories are relevant to the user's current task via memory(action="read", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite
for all subsequent work.

Do NOT attempt to perform the exploration yourself — it will exhaust your context
window. The subagent handles it in isolation.

If the subagent fails, report the error to the user. Do NOT fall back to exploring
inline — suggest the user re-run onboarding or check the MCP server status.

If you cannot spawn subagents, execute the subagent prompt directly — but be aware
this will consume significant context.
```

### Subagent Prompt Structure

The `subagent_prompt` field is built server-side by concatenating:

1. **Preamble** (~5 lines): project activation instruction + subagent framing
2. **Existing onboarding prompt** (from `build_onboarding_prompt()`): all 7 exploration
   steps, gate checklist, memory templates — unchanged
3. **System prompt draft** (from `build_system_prompt_draft()`): as today
4. **Gathered project data**: hardware, model options, protected memories, workspace
   projects — everything currently in the flat response
5. **Epilogue** (~15 lines): return contract specifying what the subagent must return

#### Preamble

```
You are an onboarding subagent for codescout. Your job is to thoroughly explore
this codebase and write project memories that will be used by every future session.

FIRST ACTION: Call activate_project(".", read_only: false) to initialize the
project context. All subsequent tool calls depend on this.

Then follow the exploration and memory-writing instructions below exactly.
```

#### Epilogue (Return Contract)

```
## Return Contract

When you have completed ALL exploration steps and written ALL memories, end your
response with this structured summary:

**Exploration Summary:**
- What this system does (your own words, not the README's)
- The 5 most important types/modules (name, file, role)
- How a typical operation flows (concrete function names)
- What surprised you (things docs didn't mention)

**Memories Written:**
- List each memory topic you wrote (e.g., "architecture", "conventions", etc.)

**Warnings:**
- Any issues encountered (index not built, LSP failures, files that couldn't be read)
- Steps you couldn't fully complete and why

This summary is returned to the main agent and shown to the user. Make it
informative but concise — aim for 300-500 tokens total.

LAST ACTION: Call activate_project(".") before returning to ensure the parent's
project state is unchanged.
```

## Content Delivery Mechanism

### How `subagent_prompt` reaches the subagent

The `subagent_prompt` is a large text blob (~600+ lines). The main agent must pass
it to the Agent tool's `prompt` parameter. This means it briefly transits through
the main agent's context as part of a tool call — but it is **not internalized**
(the agent copies it into the tool parameter, it doesn't reason about it).

There is no way to make it truly opaque — MCP output buffer refs (`@tool_*`) are
session-scoped to the parent's MCP connection and inaccessible to subagents. The
"do NOT read or summarize" instruction in `main_agent_instructions` is the
behavioral guard.

### `call_content` implementation

`call_content` returns two `Content::text` blocks:

1. **Block 1:** `main_agent_instructions` — the dispatch command the main agent acts on
2. **Block 2:** `subagent_prompt` prefixed with a delimiter line:
   `--- ONBOARDING SUBAGENT PROMPT (pass as-is to subagent) ---`

The delimiter reinforces the pass-through intent. The main agent sees both blocks
but only acts on Block 1.

### `system_prompt_draft` disposition

`system_prompt_draft` moves entirely into `subagent_prompt` (item 3 in the
concatenation). It is no longer surfaced as a separate top-level field or inline
content block by `call_content`. The subagent uses it during memory writing;
the main agent does not need it.

## Rust Code Changes

All changes in `src/tools/workflow.rs` unless noted:

| Change | Description | Size |
|---|---|---|
| `build_subagent_preamble()` | New function returning the preamble string | ~15 lines |
| `build_subagent_epilogue()` | New function returning the return contract | ~15 lines |
| `build_main_agent_instructions()` | New function returning the dispatch instructions | ~20 lines |
| `Onboarding::call` response assembly | Restructure final JSON: remove `instructions` and `system_prompt_draft` from top level, add `subagent_prompt` and `main_agent_instructions` | ~20 lines modified |
| `Onboarding::call_content` | Rewrite to emit two Content::text blocks (main instructions + delimited subagent prompt) instead of current inline formatting of `instructions` and `system_prompt_draft` | ~25 lines modified |
| `format_onboarding` | Handle new response shape in compact format | ~5 lines |
| Prompt surface review | Review `server_instructions.md` for stale references to `instructions` field | ~0-5 lines |

**Total:** ~100 lines new/modified.

### What Does NOT Change

- `onboarding_prompt.md` — becomes the subagent's prompt body, content unchanged
- `workspace_onboarding_prompt.md` — unchanged
- `build_onboarding_prompt()` in `src/prompts/mod.rs` — unchanged
- `gather_project_context()` — unchanged
- Fast path (already onboarded) — identical response shape
- Companion plugin hooks — unchanged
- All existing fast-path tests — remain valid

### Prompt Surface Review

Per CLAUDE.md: "Any change to tool behavior or signatures requires a prompt surface
review." The three surfaces to check:

1. `src/prompts/server_instructions.md` — verify `onboarding` tool description does
   not reference the removed `instructions` field
2. `src/prompts/onboarding_prompt.md` — unchanged (now lives inside `subagent_prompt`)
3. `build_system_prompt_draft()` in `src/tools/workflow.rs` — unchanged (now lives
   inside `subagent_prompt`)

## Edge Cases

### Subagent failure or timeout

The main agent reports the error to the user. No silent fallback to inline
exploration — that defeats the purpose. User can re-run onboarding.

Note: the MCP `onboarding()` tool call itself returns quickly (~1-2 seconds) —
it only gathers project data and builds the prompt. The long-running work (30-50
tool calls, several minutes) happens inside the subagent, which is managed by the
host (Claude Code), not by the MCP tool. MCP request timeouts do not apply.

### `force=true` re-onboarding

Same subagent flow as first-time. The subagent overwrites existing memories.

### Workspace mode (multi-project)

The subagent prompt includes all workspace/project data from
`gather_project_context`. Per-project memory writing handled by the subagent
exactly as the current inline flow.

### `activate_project` state safety

Onboarding runs at session start before any other project activation. The subagent
calls `activate_project(".", read_only: false)` — the same project the parent has
active. The epilogue includes a restore instruction: "Call `activate_project(".")`
before returning to ensure the parent's project state is unchanged." This is
belt-and-suspenders — the state should already be correct, but Iron Law 4 requires
explicit restoration.

### Protected memory merge protocol

The existing onboarding prompt (Phase 2, "Protected Memories" section) already
contains full merge instructions: read existing content, merge with new findings,
preserve user-written sections. Since the onboarding prompt content is unchanged,
the subagent inherits this protocol. No additional work needed — verified by
inspection of `onboarding_prompt.md` lines 307-340.

### Backward compatibility

The `instructions` field is replaced by `subagent_prompt` + `main_agent_instructions`.
Any MCP client parsing `instructions` programmatically needs to adapt. Low risk —
`instructions` was always meant for LLM consumption, not programmatic use.

### MCP clients without subagent support

Clients that cannot spawn subagents will see `main_agent_instructions` telling them
to do so, which they can't. They'll need to either: (a) execute `subagent_prompt`
inline (falling back to current behavior), or (b) report that onboarding requires
subagent support. The `main_agent_instructions` includes a fallback note: "If you
cannot spawn subagents, execute the subagent prompt directly — but be aware this
will consume significant context."

## Testing

- Unit test: new response shape has `subagent_prompt` and `main_agent_instructions`
- Unit test: `subagent_prompt` contains preamble, exploration prompt, gathered data, epilogue
- Unit test: `main_agent_instructions` is present and concise
- Unit test: fast path returns unchanged shape
- Manual E2E: `cargo build --release`, restart MCP, run onboarding on a test project,
  verify main agent dispatches subagent and receives summary
