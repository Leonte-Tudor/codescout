# Query-Shape Detection for Code Search Tools

**Date:** 2026-03-25
**Status:** Draft
**Branch:** experiments

## Problem

The review in `docs/reviews/2026-03-25-codescout-session-usage-review.md` documents a
recurring failure mode: the LLM uses the wrong search tool for the query shape.

The clearest example is `find_symbol` being called with regex alternation patterns like
`preprocess_data|train|evaluate|main`. These return empty results — not because the tool
is broken, but because `find_symbol` does substring matching on symbol names, not regex.

The LLM then concludes the tool failed and escapes to host grep or abandons codescout,
when the correct fix was to reformulate the query.

A secondary problem is `search_pattern` rejecting queries like `Vec<String>` as invalid
regex when the user clearly intended a literal text search. The regex metacharacters are
incidental, not intentional.

## Design Decisions

These were explored during brainstorming:

1. **No auto-routing.** `find_symbol` will NOT internally execute `search_pattern` logic.
   A tool should do what its name says. Cross-tool execution destroys API trust.
   Instead, `find_symbol` returns a `RecoverableError` with a hint pointing to
   `search_pattern`. One extra LLM round-trip, but honest and simple.

2. **Literal fallback for `search_pattern`.** Unlike `find_symbol`, literal text search
   is still within `search_pattern`'s domain — it's a text search tool. When regex
   compilation fails and the input doesn't look like intended regex, escape the pattern
   and search literally. Disclose the fallback via `mode: "literal_fallback"`.

3. **No `next_actions`.** The spec originally proposed structured tool suggestions on
   successful responses. Deferred — the acute pain is wrong-tool-family queries, not
   lack of cross-tool guidance on successes.

4. **Single shared predicate.** One `fn is_regex_like(s: &str) -> bool` function serves
   both tools. No enum, no classifier struct. YAGNI.

## The `is_regex_like` Predicate

A pure function in `src/tools/mod.rs`:

```rust
/// Returns true if the input string looks like it was intended as a regex pattern
/// rather than a plain symbol name or literal text search.
fn is_regex_like(s: &str) -> bool
```

### Positive signals (any one triggers `true`)

- Contains `|` with alphanumeric text on both sides (alternation)
- Contains `.*`, `.+`, or `.?` (quantified wildcard)
- Starts with `^` or ends with `$` (anchors)
- Contains `[` with a `-` range inside before `]` (character class like `[A-Z]`, not `[u8]`)
- Contains regex escape sequences: `\b`, `\w`, `\d`, `\s`
- Contains `(` followed later by `)` (grouping)

### Does NOT trigger for

- Lone `|` at start or end of string
- Slashes in name paths (`MyStruct/my_method`)
- Underscores, hyphens, dots in identifiers
- Normal punctuation that isn't regex-specific
- Angle brackets (`<`, `>`) — common in type names like `Vec<String>`
- Square brackets without a `-` range inside — `[u8]`, `[i32; 4]` are Rust types, not
  character classes

### Known false positives

The `(...)` rule may trigger on rare inputs like `HashMap(String, Vec)`. In practice,
parenthesized text is not a common `find_symbol` query — symbol names use `::` or `/`,
not function-call syntax. This is an accepted trade-off: the false positive returns a
helpful `RecoverableError` with a corrective hint, not a silent failure.

### Testing

Pure function — unit tested with a table of `(input, expected)` pairs:

| Input | Expected | Reason |
|-------|----------|--------|
| `"foo\|bar"` | true | alternation |
| `"foo\|bar\|baz"` | true | multi-alternation |
| `"foo.*bar"` | true | wildcard |
| `"^main"` | true | start anchor |
| `"name$"` | true | end anchor |
| `"[A-Z]foo"` | true | character class with range |
| `"\\bword"` | true | word boundary |
| `"my_function"` | false | plain identifier |
| `"MyStruct/method"` | false | name_path style |
| `"some-name"` | false | hyphenated identifier |
| `"CamelCase"` | false | normal symbol name |
| `"\|leading"` | false | lone pipe at start |
| `"trailing\|"` | false | lone pipe at end |
| `"foo.bar"` | false | dotted identifier (no quantifier) |
| `"Vec<String>"` | false | generic type syntax |
| `"[u8]"` | false | Rust slice type, no range |
| `"[i32; 4]"` | false | Rust array type, no range |
| `""` | false | empty string |
| `"some(thing)"` | true | accepted false positive (grouping) |
## `find_symbol` — Regex Pattern Rejection

### Where

Early in `FindSymbol::call` (`src/tools/symbol.rs`), after parameter extraction but
before any LSP work.

### Logic

```rust
if !is_name_path && is_regex_like(pattern) {
    return Err(RecoverableError::with_hint(
        format!(
            "pattern looks like a regex (found '{}') — find_symbol searches symbol names, not text",
            // include the specific trigger character/sequence
        ),
        "Use search_pattern(pattern=\"...\") for regex text search, \
         or make separate find_symbol calls for each symbol name",
    ).into());
}
```

### Conditions

- Only when `pattern` is used, not `name_path` — exact name_path lookups skip this check
- Only when `is_regex_like(pattern)` returns true
- Returns before any LSP/tree-sitter work — no wasted computation

### What doesn't change

Everything after the guard: LSP queries, tree-sitter fallback, overflow, body cap,
`by_file` distribution — all untouched.

### Tests

| Scenario | Expected |
|----------|----------|
| Plain substring pattern (e.g. `"my_func"`) | Normal symbol results |
| `name_path` with slashes | Normal lookup, no regex check |
| Pattern with `\|` alternation | `RecoverableError` mentioning `search_pattern` |
| Pattern with `.*` | `RecoverableError` |

## `search_pattern` — Literal Fallback on Invalid Regex

### Where

In `SearchPattern::call` (`src/tools/file.rs`), in the regex compilation error path
(the `.map_err` block on `RegexBuilder::new(pattern).build()`).

### Current behavior

Regex fails to compile → `RecoverableError` always.

### New behavior

```
regex fails to compile:
  if is_regex_like(pattern):
    → existing RecoverableError (user meant regex, it's just broken)
  else:
    → escape pattern with regex::escape()
    → compile escaped pattern (infallible)
    → run normal search loop
    → return results with mode: "literal_fallback"
```

### Response shape

**Literal fallback:**
```json
{
  "mode": "literal_fallback",
  "reason": "pattern was not valid regex — searched as literal text",
  "matches": [...],
  "total": 5
}
```

**Normal search** (no change): no `mode` field present. Absence of `mode` means
"normal regex search." This avoids changing the response contract for existing behavior.

### Compact rendering

The `format_compact` implementation (`format_search_pattern` in `src/tools/file.rs`)
should prepend `[literal fallback]` to the output when `mode` is present, so the LLM
sees the fallback even in compact rendering.

### Overflow

Literal fallback reuses the same overflow logic as normal search — same walk-and-match
loop, same cap, same hint format. No special handling needed.

### Tests

| Scenario | Expected |
|----------|----------|
| Valid regex | Normal results, no `mode` field |
| Invalid regex with regex-like intent (e.g. `"(unclosed"` + other signals) | Existing `RecoverableError` |
| Invalid regex, looks like plain text (e.g. `"Vec<String>"`) | Literal fallback, `mode: "literal_fallback"`, matches found |
| Literal fallback with zero matches | `mode: "literal_fallback"`, empty matches, `total: 0` |
| Literal fallback hitting cap | Same overflow semantics as normal mode |

## Prompt Surface Updates

Three files, all small additions.

### `src/prompts/server_instructions.md`

- Clarify `find_symbol` description: `pattern` is a substring/exact name, not regex
- Add one anti-pattern row to the existing anti-patterns table:

| Don't | Do instead | Why |
|-------|------------|-----|
| `find_symbol(pattern="foo\|bar")` | `search_pattern(pattern="foo\|bar")` or separate `find_symbol` calls | `find_symbol` rejects regex-like patterns |

### `src/prompts/onboarding_prompt.md`

In **Step 6: Code Exploration by Concept** (line ~192), after the paragraph about
`semantic_search` vs `search_pattern`, add:

> `find_symbol` searches by symbol name substring — use `search_pattern` for
> regex/text discovery. Do not pass regex alternation (`foo|bar`) to `find_symbol`.

### `build_system_prompt_draft()` in `src/tools/workflow.rs`

In the single-project Navigation Strategy section (around line 730), after the
`find_symbol` entry (`"4. find_symbol..."`), add one line:

> regex-like patterns belong in `search_pattern`, not `find_symbol`
## Impact

- **~30-40 lines** of new production code (predicate + two guard blocks)
- **No schema change** — no new parameters, no changed parameter semantics
- **No breaking change for valid queries** — only regex-like patterns in `find_symbol`
  see different behavior (error instead of empty results)
- **`search_pattern` gains a new response shape** for literal fallback, but existing
  valid-regex responses are unchanged
- **Net reduction in wasted LLM round-trips** — regex in `find_symbol` gets caught
  immediately with a corrective hint; literal text in `search_pattern` gets results
  instead of an error
