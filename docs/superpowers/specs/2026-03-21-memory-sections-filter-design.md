# Design: `sections` filter on `memory(action="read")`

**Date:** 2026-03-21
**Status:** Approved

## Problem

In a multi-language workspace, `language-patterns` is a single markdown file with one
`### Language` section per language. An agent working on a specific sub-project reads the
whole file and receives patterns for every language — most of which are irrelevant to its
task and waste context window tokens.

Agents already know which language they work in (each sub-project has a single primary
language), so they can declare what they need. There is no mechanism today to request a
subset of the memory content.

## Goal

Allow agents to pass a `sections` filter when reading any memory topic so that only the
requested `### Heading` blocks are returned. The feature is general (works for any
structured memory file), but the primary driver is `language-patterns`.

## Non-Goals

- Splitting `language-patterns` into per-language sub-topics (rejected: loses the shared
  overview, changes writing convention).
- Special-casing a `languages` param for `language-patterns` only (rejected: arbitrary
  special case, same implementation cost as the general solution).
- Filtering on `##` (H2) or `#` (H1) headings — H3 (`###`) is the established section
  delimiter in codescout memories.

## Design

### Schema change

Add an optional `sections` param to the **`Memory` struct's `input_schema`** (the unified
`memory` tool, `action="read"` branch). The legacy `ReadMemory` struct registered
separately is not updated.

```json
"sections": {
  "type": "array",
  "items": { "type": "string" },
  "description": "Return only the listed ### headings (case-insensitive). E.g. [\"Rust\", \"TypeScript\"]. Omit to return full content."
}
```

When `sections` is absent or empty the tool behaves exactly as today (full content
returned). **`Memory::call` checks `sections.is_empty()` and skips filtering entirely
before calling `filter_sections`. `filter_sections` has a precondition that `sections` is
non-empty.**

### Filtering logic — `src/memory/filter.rs`

A pure function:

```rust
pub fn filter_sections(content: &str, sections: &[&str]) -> FilterResult
```

Precondition: `sections` is non-empty (enforced by the caller).

```rust
pub struct FilterResult {
    pub content: String,           // filtered markdown (preamble + matched section bodies)
    pub matched: bool,             // true if at least one requested section was found
    pub missing: Vec<String>,      // requested sections not found — in caller-supplied casing
    pub available: Vec<String>,    // all ### headings present in the file, in order, normalized
}
```

**Algorithm:**

1. Scan lines, split into blocks at each `### ` (H3) boundary. A boundary is any line
   that **starts with exactly `### ` on the raw line — no leading whitespace is stripped
   before the check.** An indented ` ### Rust` is not a section boundary.
2. The preamble — all lines before the first `### ` — is collected separately and always
   prepended to the output (it provides context such as title and description).
3. A section starts at a `### ` line and ends immediately before the next `### ` line (or
   EOF). **All lines between one `### ` boundary and the next are part of that section's
   body, including any `####` sub-headings or deeper nesting.**
4. Heading text is extracted from a `### ` line by stripping the `### ` prefix, then
   trimming both leading and trailing whitespace (handles malformed headings such as
   `###  Rust` or `### Rust  `). This normalized form is used for both matching and for
   populating `available`.
5. Collect `available` — the **normalized heading text** (post-trim) of every `### ` block
   found in the file, in order.
6. A section matches if its normalized heading text equals any requested section,
   compared case-insensitively. **If the file contains duplicate `### ` headings with the
   same name, all matching blocks are included in the output.**
7. Build `missing` from requested sections that had no match, **preserving the
   caller-supplied casing** (e.g. if the caller passed `"typescript"`, `missing` contains
   `"typescript"`, not `"TypeScript"`).
8. Set `matched = true` if at least one block was included.
9. Return `FilterResult` (always — never `Option`). The caller checks `matched`.

### Integration in `Memory::call` (read arm)

This applies to **both** the `private=true` and `private=false` branches of the read arm.
After fetching content from `MemoryStore::read`:

1. Parse `sections` from input. If absent or empty, skip filtering entirely (return full
   content as today). An explicitly passed empty array `sections=[]` is treated identically
   to omitting the param.
2. Call `filter_sections(content, &sections)`.
3. If `!result.matched`: return `RecoverableError` whose hint says
   `"available sections: <result.available.join(", ")>"`. If `available` is empty (file
   has no `### ` headings), the hint says `"this memory has no ### sections to filter"`.
4. If `result.matched`: replace `content` with `result.content` for all downstream
   handling. The inline-vs-buffer threshold check is applied to the filtered content.
   - When buffering, pass a **synthetic path** (e.g. `format!("{} (filtered)", topic)`)
     to `store_file` rather than the real file path, so that agents paginating via
     `read_file("@file_ref")` receive the filtered content, not the original full file.
   - If `result.missing` is non-empty, include `"missing": [...]` in the JSON response
     alongside `"content"` or `"file_id"` (see response shapes below).

### Response shapes

Full content, no filter:
```json
{ "content": "..." }
```

Filtered, all found, fits inline:
```json
{ "content": "### Rust\n..." }
```

Filtered, partial match, fits inline:
```json
{ "content": "### Rust\n...", "missing": ["TypeScript"] }
```

Filtered, partial match, content exceeds inline limit (buffered):
```json
{ "file_id": "@file_abc", "total_lines": 120, "missing": ["TypeScript"] }
```

No match (`RecoverableError`):
```json
{ "error": "no sections matched", "hint": "available sections: Rust, Python" }
```

### Prompt surfaces

`src/prompts/server_instructions.md` is updated to document the `sections` param.
`src/prompts/onboarding_prompt.md` and `build_system_prompt_draft()` in
`src/tools/workflow.rs` do not reference `memory` tool params today and need no update.

## Testing

Unit tests live in `src/memory/filter.rs`. Integration tests in `src/tools/memory.rs`.

| Test | What it covers |
|------|----------------|
| `filter_sections_returns_matching_section` | Basic happy path — one section returned with preamble |
| `filter_sections_case_insensitive` | `"rust"` matches `### Rust` |
| `filter_sections_multiple_sections` | `["Rust", "TypeScript"]` returns both blocks |
| `filter_sections_preserves_preamble` | Lines before first `### ` are always included |
| `filter_sections_no_match_returns_not_matched` | Unknown section → `matched=false`, `available` populated |
| `filter_sections_partial_match_returns_missing` | One found, one not → `matched=true`, `missing` in caller-supplied casing |
| `filter_sections_duplicate_headings_both_included` | Two `### Rust` blocks → both returned |
| `filter_sections_nested_h4_included_in_body` | `####` sub-heading inside a matched section is part of the body |
| `filter_sections_heading_whitespace_normalized` | `###  Rust` (double space) matches `"rust"`; appears as `"Rust"` in `available` |
| `filter_sections_no_headings_in_file_returns_not_matched` | File with no `### ` headings → `matched=false`, `available` is empty |
| `filter_sections_indented_heading_not_a_boundary` | ` ### Rust` (leading space) is not treated as a section boundary |
| `filter_sections_empty_sections_is_caller_error` | Documents the precondition: caller must not pass empty slice |
| `memory_read_sections_filter_integration` | Full `Memory::call` path with `sections` param on shared memory; empty `sections=[]` returns full content |
| `memory_read_sections_filter_private_integration` | Same filtering behavior applies in `private=true` branch — verifies both code paths share the logic |

## Files Changed

| File | Change |
|------|--------|
| `src/memory/filter.rs` | New — `filter_sections` function, `FilterResult` struct, unit tests |
| `src/memory/mod.rs` | Add `pub mod filter;` |
| `src/tools/memory.rs` | Add `sections` param to `Memory` input schema; call `filter_sections` in both branches of read arm; integration tests |
| `src/prompts/server_instructions.md` | Document `sections` param in the `memory` tool reference |
