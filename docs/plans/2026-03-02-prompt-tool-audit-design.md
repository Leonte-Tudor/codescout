# Design: Prompt & Tool Audit — March 2026

## Problem

After auditing registered tools against `server_instructions.md`, `system-prompt.md`,
`CLAUDE.md`, and `onboarding_prompt.md`, three categories of issues were found:

1. **Noise**: `get_usage_stats` is listed in agent-facing system instructions but is a
   maintainer/debugging tool — agents should not be guided toward it.
2. **Missing behavior**: `@file_*` handles auto-refresh when the underlying file changes
   on disk, but this is never surfaced to the LLM.
3. **Staleness**: Tool counts (31/30 → 32) and one renamed tool call
   (`get_symbols_overview` → `list_symbols`) are wrong in several files.

## Non-Goals

- Documenting `@ack_*` and `@file_*` in `server_instructions.md` — these are
  self-documented in tool schemas; no over-emphasis needed.
- Fixing stale line numbers in `system-prompt.md` — they will drift again;
  tool count is sufficient.
- Auditing `onboarding_prompt.md` beyond the confirmed stale reference.

## Design

### 1. Remove `get_usage_stats` from `server_instructions.md`

Remove the single bullet from the "Project Management" section:

```
- `get_usage_stats` — per-tool call counts, error rates, latency percentiles
```

**Rationale:** The tool is self-describing via its schema. Removing it keeps the
system prompt lean and avoids nudging agents toward self-analytics.

### 2. `@file_*` refresh indicator in `run_command` output

**Change 1 — `OutputBuffer::resolve_refs` return type**

Extend from `(String, Vec<TempFile>, bool)` to
`(String, Vec<TempFile>, bool, Vec<String>)` where the new `Vec<String>` is the
list of handle IDs (`@file_abc123`) that were refreshed from disk during resolution.

**Change 2 — `RunCommand::call` prepend note**

After `resolve_refs`, if any handles were refreshed, prepend one line per refreshed
handle to the command output:

```
↻ @file_abc123 refreshed from disk (file changed since last read)
```

This appears before grep/sed results so the LLM sees it as context, not as
part of the command output itself.

**Scope:** Only `RunCommand::call` calls `resolve_refs`, so the signature change
is contained to `src/tools/output_buffer.rs` and `src/tools/workflow.rs`.

### 3. Documentation staleness fixes

| File | Change |
|------|--------|
| `server_instructions.md` | Remove `get_usage_stats` bullet (covered in §1) |
| `.code-explorer/system-prompt.md` | `"interface for all 31 tools"` → `"interface for all 32 tools"` |
| `CLAUDE.md` | `"30 tools registered"` → `"32 tools registered"` |
| `src/prompts/onboarding_prompt.md` | `get_symbols_overview("src")` → `list_symbols("src/")` |

## Files Touched

- `src/tools/output_buffer.rs` — extend `resolve_refs` return type, track refreshed handles
- `src/tools/workflow.rs` — consume refreshed list, prepend indicator lines
- `src/prompts/server_instructions.md` — remove `get_usage_stats` bullet
- `src/prompts/onboarding_prompt.md` — fix `get_symbols_overview` → `list_symbols`
- `.code-explorer/system-prompt.md` — fix tool count
- `CLAUDE.md` — fix tool count
