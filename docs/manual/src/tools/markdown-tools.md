# Markdown Tools: read_markdown & edit_markdown

Two dedicated tools for navigating and editing Markdown files using
heading-based addressing. They replace the need to read raw line ranges
or construct fragile string replacements against unstructured text.

---

## read_markdown

Navigate a Markdown file by heading. Without `heading`/`headings` params,
returns a **heading map** — the document outline with line numbers.

### Parameters

| Param | Type | Description |
|---|---|---|
| `path` | string | Markdown file path (relative to project root) |
| `heading` | string | Single section to read (fuzzy matched) |
| `headings` | string[] | Multiple sections in one call (mutually exclusive with `heading`) |
| `start_line` / `end_line` | int | Raw line range fallback (1-indexed, inclusive) |

### Usage

```
// Step 1: get the heading map
read_markdown("docs/guide.md")
→ heading map with line numbers

// Step 2: read specific sections
read_markdown("docs/guide.md", headings=["## Auth", "## Config"])
→ both sections in one response
```

The heading map is the starting point for any markdown edit workflow — always
read it first so you know which headings exist before targeting one.

---

## edit_markdown

Edit a Markdown document section by heading. Heading matching is **fuzzy** —
`## Auth` matches `## Authentication` — so you don't need to quote headings
exactly.

### Actions

| Action | Description |
|---|---|
| `replace` | Replace section body (heading line is preserved) |
| `insert_before` | Insert content before the heading |
| `insert_after` | Insert content after the section (before next heading) |
| `remove` | Delete the section and its body |
| `edit` | Surgical string replacement within a section (`old_string` → `new_string`) |

### Parameters

| Param | Type | Description |
|---|---|---|
| `path` | string | Markdown file path |
| `heading` | string | Target section heading (fuzzy matched) |
| `action` | string | One of the actions above |
| `content` | string | New body for `replace`/`insert_*` (heading not included) |
| `old_string` | string | For `edit`: exact text to find |
| `new_string` | string | For `edit`: replacement text |
| `replace_all` | bool | For `edit`: replace all occurrences (default: false) |
| `edits` | array | Batch mode — multiple operations applied atomically |

### Examples

```
// Replace a section body
edit_markdown("docs/guide.md",
  heading="## Configuration",
  action="replace",
  content="See project.toml for all options.\n")

// Surgical fix inside a section
edit_markdown("docs/guide.md",
  heading="## Auth",
  action="edit",
  old_string="secret_key = \"\"",
  new_string="secret_key = \"<your-key>\"")

// Batch: two edits in one atomic call
edit_markdown("docs/guide.md",
  edits=[
    { heading: "## Usage", action: "replace", content: "..." },
    { heading: "## License", action: "remove" }
  ])
```

### Batch Mode

Pass an `edits` array instead of `heading`/`action` to apply multiple operations
atomically. All edits are validated before any are applied — if one heading is
missing, nothing changes.

---

## Why Not edit_file?

`edit_file` works on raw strings and requires exact whitespace/newline matching.
For Markdown, heading-scoped edits are both safer and more resilient:

| Scenario | edit_file | edit_markdown |
|---|---|---|
| Replace a section body | Error-prone: must match surrounding blank lines exactly | `action=replace` — heading preserved automatically |
| Edit text inside a section | Works, but edits anywhere in the file | `action=edit` scoped to one section |
| Remove a section | Must know exact start/end lines | `action=remove` — no line numbers needed |
| Multiple edits | Multiple calls, each can conflict | `edits=[]` batch — atomic |
