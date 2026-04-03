# Onboarding Buffered Output with Client-Aware Dispatch

**Date:** 2026-03-29
**Status:** Draft
**Scope:** `src/tools/workflow.rs` (Onboarding tool), `src/tools/mod.rs` (ToolContext)

## Problem

`Onboarding::call_content()` overrides the default `Tool::call_content()` and returns two raw `Content::text` blocks totaling 60-80KB for multi-project workspaces. This bypasses the `@tool_*` buffer system that all other tools use for large output.

Consequences:
- **Claude Code**: persisted-output mechanism kicks in, saves to a `toolu_xxx.json` file, model sees only a 2KB preview with no `@ref` handle. The model must discover the file path and manually `cat` it in chunks.
- **Other MCP clients**: may dump the full response into context (wasting tokens), truncate silently, or error out.
- **Multi-project workspaces**: the problem scales linearly with project count (per-project protected memories, workspace phases, language patterns).

## Design

### Core Principle

Always buffer the subagent prompt via `OutputBuffer::store_tool()`. Return a compact response (~1-2KB) with the buffer ref and client-appropriate instructions for consuming it.

### Client Detection

Detect the MCP client from `ToolContext::peer`, which is already available:

```rust
fn detect_client_name(ctx: &ToolContext) -> Option<String> {
    ctx.peer.as_ref()
        .and_then(|p| p.peer_info())
        .map(|info| info.client_info.name.to_lowercase())
}

fn is_subagent_capable(client_name: Option<&str>) -> bool {
    client_name.map_or(false, |n| n.contains("claude"))
}
```

This is intentionally conservative: only Claude Code gets subagent instructions. Unknown clients get single-agent instructions. As other clients gain subagent support (Gemini CLI, etc.), add them to the detection.

### Response Structure

`call_content()` returns a single `Content::text` block containing structured JSON:

```json
{
  "output_id": "@tool_abc123",
  "summary": "[rust, python] - workspace (3 projects) - onboarding required",
  "instructions": "... client-appropriate instructions ...",
  "hint": "read_file(\"@tool_abc123\", start_line=1, end_line=50) to start reading"
}
```

### Client-Aware Instructions

**For Claude Code** (subagent-capable):

```
Onboarding required. The full onboarding prompt is stored in @tool_xxx.

Spawn a general-purpose subagent with model=sonnet to perform onboarding.
The subagent must:
1. Call read_file("@tool_xxx") to get the onboarding prompt
2. Follow the prompt instructions to explore the codebase and write memories

Do NOT read the onboarding prompt yourself. Pass the ref to the subagent.
```

**For all other clients** (Cursor, Copilot, Windsurf, etc.):

```
Onboarding required. The full onboarding prompt is stored in @tool_xxx.

Read it with: read_file("@tool_xxx", start_line=1, end_line=100)
Then follow the instructions to explore the codebase and write project memories.
Use pagination (start_line/end_line) to read in chunks if needed.
```

### Affected Code Paths

Three paths in `call_content()` currently return a `subagent_prompt`:

1. **Full onboarding** (first time or `force: true`) - lines 1688-1704
2. **Version refresh** (stale `onboarding_version`) - same block, triggered by version check
3. **Explicit refresh** (`refresh_prompt: true`) - same block

All three get the same treatment: buffer the prompt, return compact response with ref.

### What Does NOT Change

- `call()` - unchanged, still builds the full JSON with `subagent_prompt` field. Direct callers (tests, programmatic use) still get the full response.
- Side effects (config creation, memory writes, workspace.toml) - unchanged.
- `format_onboarding()` - still used for the compact summary line.
- Already-onboarded fast path - no `subagent_prompt`, no buffering needed.

### Buffer Lifetime

`@tool_*` refs live in the session-scoped `OutputBuffer` (LRU, max 20 entries). The onboarding prompt is read once at session start. At 20 entries with LRU eviction, the ref will persist for the entire onboarding flow. No lifetime concerns.

### Multi-Project Workspace Scaling

A workspace with N projects adds ~2-5KB per project to the subagent prompt (protected memories, per-project onboarding data, workspace phases). For a 6-project workspace:

| Component | Size |
|---|---|
| Onboarding template | ~28KB |
| System prompt draft | ~5KB |
| Per-project data (6 projects) | ~15-30KB |
| Hardware/model options | ~2KB |
| **Total** | **~50-65KB** |

With buffering, the main agent sees ~1-2KB regardless. The consuming agent (subagent or self) reads from the buffer at its own pace.

## Testing

1. **Unit test**: `call_content` returns structured JSON with `output_id` (not raw text blocks) when `subagent_prompt` is present.
2. **Unit test**: Claude client detection returns subagent instructions; unknown client returns single-agent instructions.
3. **Unit test**: Buffer ref is queryable via `read_file("@tool_xxx")`.
4. **Integration test**: Full onboarding flow with buffered output, verifying the ref contains the complete prompt.
5. **Existing tests**: All current `call_content` tests for version refresh and force onboarding must still pass (adapted for new response shape).

## Migration

No migration needed. The `call()` return value is unchanged, so any programmatic consumers are unaffected. Only the MCP-facing `call_content()` response changes shape. The `main_agent_instructions` field is no longer a separate key in the JSON response to the client - it's folded into the `instructions` field of the buffered response.
