# Memory Sections Filter


`memory(action="read")` now accepts a `sections` parameter that returns only the
`### Heading` blocks you need, instead of the full memory file.

## Usage

```json
{
  "action": "read",
  "topic": "language-patterns",
  "sections": ["Rust", "TypeScript"]
}
```

The response contains the file preamble (text before the first `### ` heading) plus
each matched section in file order. Heading matching is case-insensitive.

## Why this matters

Large memory files like `language-patterns` can hold hundreds of lines across
many languages. Passing `sections` slashes context cost when you only need one
language's patterns — the server does the filtering before the result reaches
the model.

## Parameters

| Parameter | Type | Notes |
|-----------|------|-------|
| `topic` | string | Memory topic to read (required). |
| `sections` | string[] | One or more `### Heading` names to return. Omit to return the full file. |
| `private` | boolean | `true` to read from the gitignored private store. |

## Return value

When `sections` is supplied and at least one heading matches, the response
contains the filtered content. When a heading is requested but not found, a
`RecoverableError` is returned that lists:

- **`missing`** — requested section names that had no match.
- **`available`** — all section headings present in the file, so you can correct the call.

```json
{
  "error": "sections not found: [\"Go\"]",
  "missing": ["Go"],
  "available": ["Rust", "TypeScript", "Python"]
}
```

## Limitations

- Only `### ` (H3) headings are treated as section boundaries. `##` and `#`
  headings are part of the preamble or carried inside a section body.
- Sections are returned in **file order**, not request order.
- The filter applies to topic-based reads only (`action="read"`). Semantic
  recall (`action="recall"`) is unaffected.
