# Gotchas & Known Issues

## Prompt Surface Consistency
The project has three prompt surfaces that reference tool names:
- `src/prompts/server_instructions.md` — injected every MCP request
- `src/prompts/onboarding_prompt.md` — one-time onboarding
- `build_system_prompt_draft()` in `src/tools/workflow.rs` — generated per-project

**When tools get renamed/consolidated, all three need coordinated updates.**
Files closer to the change get updated; distant ones get stale refs ("distance from change" problem).

## find_symbol body truncation
`find_symbol(include_body=true)` uses LSP `workspace/symbol` which returns the *name position* (single line), not the full declaration range. This causes `start_line == end_line` and a body containing only the signature.

**Workaround:** Use `list_symbols(path)` first to get correct line ranges, then fetch the body with `find_symbol(name_path=..., include_body=true)` for methods (which get full bodies from `documentSymbol`). For top-level functions in large files, `list_functions(path)` gives accurate spans.

## PreToolUse Hook Limitations
Claude Code only runs PreToolUse hooks for tools that enter permission evaluation. Built-in read-only tools (`Read`, `Glob`, `Grep`) are pre-approved and skip that pipeline entirely — hooks on them never fire. Only `Bash` is fully blockable via PreToolUse.

## No Echo in Write Responses
Mutation tools must return `json!("ok")` — never echo content back. The caller already knows the path and content; reflecting them wastes tokens.