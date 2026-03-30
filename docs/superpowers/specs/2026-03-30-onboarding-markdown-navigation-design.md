# Onboarding Prompt: Markdown-Based Section Navigation

**Date:** 2026-03-30
**Status:** Draft
**Scope:** `src/tools/workflow.rs` (Onboarding `call_content`), `src/prompts/onboarding_prompt.md`
**Builds on:** `2026-03-29-onboarding-buffered-output-design.md`

## Problem

The buffered output fix (2026-03-29) solved the 60KB context problem by storing the prompt in a `@tool_*` buffer. However, agents reading the buffer via `read_file` with line-based pagination skip lines — they lose track of their position when other tool calls intervene. Manual pagination is inherently error-prone for LLMs.

Additionally, the onboarding prompt doesn't mention `read_markdown` or `edit_markdown`, causing subagents to miss these tools when working with `.md` files.

## Design

### Core Change: File Instead of Buffer

Write the onboarding prompt to `.codescout/tmp/onboarding-prompt.md` instead of storing it in the `@tool_*` buffer. The agent navigates it with `read_markdown` heading-based navigation — no line numbers to track.

### File Location and Lifecycle

- Path: `.codescout/tmp/onboarding-prompt.md`
- Create `.codescout/tmp/` directory if it doesn't exist
- Overwrite on every onboarding call that produces a prompt (force, version refresh, explicit refresh)
- No cleanup — file is small (~30-60KB), ephemeral, and `.codescout/` is gitignored

### Response Shape

`call_content()` returns a single `Content::text` block with structured JSON:

```json
{
  "prompt_path": ".codescout/tmp/onboarding-prompt.md",
  "summary": "[rust, python] · workspace (3 projects)",
  "sections": [
    "## THE IRON LAW (27 lines)",
    "## Phase 0.5: Embedding Model Selection (40 lines)",
    "## Phase 1: Explore the Code (136 lines)",
    "## Phase 2: Write the Memories (270 lines)",
    "## After Everything Is Created (40 lines)",
    "## Gathered Project Data (6 lines)"
  ],
  "instructions": "..."
}
```

### Heading Map Construction

Parse the prompt string for level-2 headings (`## `), compute line counts between them, format as `"## Heading Name (N lines)"`. This is a simple string scan — no markdown parser needed.

```rust
fn build_heading_map(prompt: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt.lines().collect();
    let mut headings: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            headings.push((line.to_string(), i));
        }
    }
    headings.iter().enumerate().map(|(idx, (heading, start))| {
        let end = headings.get(idx + 1).map(|(_, s)| *s).unwrap_or(lines.len());
        format!("{} ({} lines)", heading, end - start)
    }).collect()
}
```

### Client-Aware Instructions

**Claude Code (subagent-capable):**

```
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform onboarding.
The subagent must read the onboarding prompt section by section:
  read_markdown(".codescout/tmp/onboarding-prompt.md")  — heading map
  read_markdown(".codescout/tmp/onboarding-prompt.md", heading="## Phase 1: Explore the Code")

Do NOT read the entire prompt at once — navigate by heading.

When the subagent completes, report its summary. Then read relevant
memories via memory(action="read", topic=...).
```

**Other clients (single-agent):**

```
Onboarding required — this project has not been explored yet.

Read the onboarding prompt section by section:
  read_markdown(".codescout/tmp/onboarding-prompt.md")  — heading map
  read_markdown(".codescout/tmp/onboarding-prompt.md", heading="## Phase 1: Explore the Code")

Follow the instructions to explore the codebase and write project memories.
Navigate by heading — do NOT read the entire file at once.
```

### Markdown Tool Mentions in Onboarding Prompt

Add `read_markdown` and `edit_markdown` to the tool reference section in `src/prompts/onboarding_prompt.md`. Specifically, in the preamble where tools are listed, add:

- `read_markdown(path)` — heading map for .md files
- `read_markdown(path, heading="## Section")` — read specific section
- `edit_markdown(path, heading, content)` — edit a markdown section

### What Changes

| Component | Before (buffer) | After (file) |
|---|---|---|
| Storage | `output_buffer.store_tool()` | `fs::write()` to `.codescout/tmp/` |
| Response key | `output_id: "@tool_xxx"` | `prompt_path: ".codescout/tmp/onboarding-prompt.md"` |
| Navigation | `read_file("@tool_xxx", start_line, end_line)` | `read_markdown(path, heading="...")` |
| First read | Line-paginated content | Heading map (sections + line counts) |

### What Does NOT Change

- `call()` — unchanged, still builds the full JSON with `subagent_prompt`
- Already-onboarded fast path — no prompt, no file written
- Client detection (`client_name`, `is_subagent_capable`) — unchanged
- Three code paths (full, version refresh, explicit refresh) — all write to same file

## Testing

1. **Unit test**: `call_content` returns JSON with `prompt_path` (not `output_id`)
2. **Unit test**: File exists at `.codescout/tmp/onboarding-prompt.md` after call
3. **Unit test**: `sections` array contains expected heading names
4. **Unit test**: `build_heading_map` correctly counts lines between headings
5. **Unit test**: Claude client gets subagent instructions; other clients get single-agent instructions (updated for `read_markdown` references)
6. **Existing tests**: Adapted from buffer-based assertions to file-based assertions
