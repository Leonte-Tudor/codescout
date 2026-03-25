# Query-Shape Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect regex-like patterns in `find_symbol` (reject with hint) and invalid-but-literal patterns in `search_pattern` (fallback to literal search), reducing wasted LLM round-trips from wrong-tool-family queries.

**Architecture:** A single shared `is_regex_like()` predicate in `src/tools/mod.rs` serves both tools. `find_symbol` calls it early to reject regex input with a `RecoverableError`. `search_pattern` calls it in the regex-compilation error path to decide between error (broken regex) and literal fallback (plain text with metacharacters).

**Tech Stack:** Rust, `regex::escape()`, existing `RecoverableError` pattern.

**Spec:** `docs/superpowers/specs/2026-03-25-query-shape-detection-design.md`

---

### Task 1: Add `is_regex_like` predicate with tests

**Files:**
- Modify: `src/tools/mod.rs` — add function before the `#[cfg(test)]` block at line 475
- Modify: `src/tools/mod.rs` — add tests in the existing `mod tests` block

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `src/tools/mod.rs`:

```rust
#[test]
fn is_regex_like_detects_alternation() {
    assert!(is_regex_like("foo|bar"));
    assert!(is_regex_like("foo|bar|baz"));
}

#[test]
fn is_regex_like_detects_wildcards() {
    assert!(is_regex_like("foo.*bar"));
    assert!(is_regex_like("foo.+bar"));
    assert!(is_regex_like("foo.?bar"));
}

#[test]
fn is_regex_like_detects_anchors() {
    assert!(is_regex_like("^main"));
    assert!(is_regex_like("name$"));
}

#[test]
fn is_regex_like_detects_character_classes_with_range() {
    assert!(is_regex_like("[A-Z]foo"));
    assert!(is_regex_like("bar[0-9]"));
}

#[test]
fn is_regex_like_detects_escape_sequences() {
    assert!(is_regex_like(r"\bword"));
    assert!(is_regex_like(r"foo\d+"));
    assert!(is_regex_like(r"\w+bar"));
    assert!(is_regex_like(r"foo\s"));
}

#[test]
fn is_regex_like_detects_grouping() {
    assert!(is_regex_like("(foo|bar)"));
    assert!(is_regex_like("some(thing)"));
}

#[test]
fn is_regex_like_rejects_plain_identifiers() {
    assert!(!is_regex_like("my_function"));
    assert!(!is_regex_like("MyStruct/method"));
    assert!(!is_regex_like("some-name"));
    assert!(!is_regex_like("CamelCase"));
    assert!(!is_regex_like("foo.bar"));
    assert!(!is_regex_like("Vec<String>"));
    assert!(!is_regex_like(""));
}

#[test]
fn is_regex_like_rejects_lone_pipe() {
    assert!(!is_regex_like("|leading"));
    assert!(!is_regex_like("trailing|"));
}

#[test]
fn is_regex_like_rejects_brackets_without_range() {
    assert!(!is_regex_like("[u8]"));
    assert!(!is_regex_like("[i32; 4]"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout is_regex_like`
Expected: compilation error — `is_regex_like` not defined

- [ ] **Step 3: Implement `is_regex_like`**

Add above the `#[cfg(test)]` block in `src/tools/mod.rs`:

```rust
/// Returns true if the input looks like it was intended as a regex pattern
/// rather than a plain symbol name or literal text.
pub(crate) fn is_regex_like(s: &str) -> bool {
    // Alternation: `foo|bar` but not `|leading` or `trailing|`
    if s.contains('|') {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.iter().filter(|p| !p.is_empty()).count() >= 2 {
            return true;
        }
    }
    // Quantified wildcard: .* .+ .?
    if s.contains(".*") || s.contains(".+") || s.contains(".?") {
        return true;
    }
    // Anchors
    if s.starts_with('^') || s.ends_with('$') {
        return true;
    }
    // Character class with range: [A-Z] but not [u8]
    // Note: only inspects the first [...] pair in the string.
    if let Some(open) = s.find('[') {
        if let Some(close) = s[open..].find(']') {
            let inside = &s[open + 1..open + close];
            if inside.contains('-') && inside.len() > 2 {
                return true;
            }
        }
    }
    // Regex escape sequences
    if s.contains(r"\b") || s.contains(r"\w") || s.contains(r"\d") || s.contains(r"\s") {
        return true;
    }
    // Grouping: ( followed by )
    if let Some(open) = s.find('(') {
        if s[open..].contains(')') {
            return true;
        }
    }
    false
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout is_regex_like`
Expected: all 9 tests PASS

- [ ] **Step 5: Run full check**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass, no warnings

- [ ] **Step 6: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat: add is_regex_like predicate for query-shape detection"
```

---
### Task 2: Add regex rejection to `find_symbol`

**Files:**
- Modify: `src/tools/symbol.rs` — add guard in `FindSymbol::call` after `is_name_path` check
- Modify: `src/tools/symbol.rs` — add tests in the existing test module (first `#[cfg(test)]` at line ~3033)

- [ ] **Step 1: Write the failing tests**

Add to a test module in `src/tools/symbol.rs`. Follow the existing pattern for
`RecoverableError` tests (see `path_not_found_hint_mentions_list_dir` at line ~3602):
construct an `Agent` with a temp dir, build a `ToolContext` via `test_ctx_with_agent`,
and downcast the `Err` to `RecoverableError`.

```rust
#[tokio::test]
async fn find_symbol_rejects_regex_alternation() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let err = FindSymbol
        .call(json!({"pattern": "foo|bar"}), &ctx)
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be RecoverableError");
    assert!(
        rec.message.contains("regex"),
        "message should mention regex, got: {}",
        rec.message
    );
    assert!(
        rec.hint.as_deref().unwrap_or("").contains("search_pattern"),
        "hint should mention search_pattern, got: {:?}",
        rec.hint
    );
}

#[tokio::test]
async fn find_symbol_rejects_regex_wildcard() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let err = FindSymbol
        .call(json!({"pattern": "foo.*bar"}), &ctx)
        .await
        .unwrap_err();

    assert!(
        err.downcast_ref::<crate::tools::RecoverableError>()
            .is_some(),
        "should be RecoverableError, got: {}",
        err
    );
}

#[tokio::test]
async fn find_symbol_allows_plain_pattern() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("test.rs"), "fn my_function() {}\n").unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    // Should NOT be rejected — plain substring
    let result = FindSymbol
        .call(json!({"pattern": "my_function"}), &ctx)
        .await;
    assert!(result.is_ok(), "plain pattern should not be rejected");
}

#[tokio::test]
async fn find_symbol_allows_name_path_with_regex_chars() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    // name_path skips regex check entirely — may return 0 symbols but not a regex error
    let result = FindSymbol
        .call(json!({"name_path": "foo|bar"}), &ctx)
        .await;
    assert!(
        result.is_ok(),
        "name_path should skip regex check, got err: {:?}",
        result.err()
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout find_symbol_rejects`
Expected: FAIL — no regex guard exists yet (the calls return `Ok` with empty symbols
instead of `Err`)

- [ ] **Step 3: Implement the guard**

In `src/tools/symbol.rs`, in `FindSymbol::call`, add after the `is_name_path` assignment
(`let is_name_path = input["name_path"].is_string();`) and before `let kind_filter`:

```rust
// Reject regex-like patterns early — find_symbol does substring matching,
// not regex. Point the LLM to search_pattern instead.
if !is_name_path && super::is_regex_like(pattern) {
    let trigger = if pattern.contains('|') {
        "'|'"
    } else if pattern.contains(".*") || pattern.contains(".+") {
        "'.*'"
    } else if pattern.starts_with('^') || pattern.ends_with('$') {
        "'^'/'$'"
    } else {
        "regex syntax"
    };
    return Err(RecoverableError::with_hint(
        format!(
            "pattern looks like a regex (found {trigger}) — \
             find_symbol searches symbol names, not text"
        ),
        "Use search_pattern(pattern=\"...\") for regex text search, \
         or make separate find_symbol calls for each symbol name",
    )
    .into());
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout find_symbol_rejects && cargo test -p codescout find_symbol_allows`
Expected: all 4 new tests PASS

- [ ] **Step 5: Run full check**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat: find_symbol rejects regex-like patterns with corrective hint"
```

---
### Task 3: Add literal fallback to `search_pattern`

**Files:**
- Modify: `src/tools/file.rs` — change the regex compilation block in `SearchPattern::call`
- Modify: `src/tools/file.rs` — update `format_search_pattern` for compact rendering
- Modify: `src/tools/file.rs` — add tests and update existing test in the test module

- [ ] **Step 1: Write the failing tests**

Add to the test module in `src/tools/file.rs`. Use `project_ctx()` (returns `(TempDir, ToolContext)`)
for tests that need files on disk. Use `test_ctx()` for tests that don't need a project root.

```rust
#[tokio::test]
async fn search_pattern_literal_fallback_on_plain_text() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(
        dir.path().join("test.rs"),
        "fn process(v: Vec<String>) {}\n",
    )
    .unwrap();
    let result = SearchPattern
        .call(json!({"pattern": "Vec<String>"}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["mode"].as_str().unwrap(), "literal_fallback");
    assert!(result["reason"].as_str().unwrap().contains("literal"));
    assert!(!result["matches"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_pattern_literal_fallback_zero_matches() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("test.rs"), "fn main() {}\n").unwrap();
    let result = SearchPattern
        .call(json!({"pattern": "Vec<String>"}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["mode"].as_str().unwrap(), "literal_fallback");
    assert!(result["matches"].as_array().unwrap().is_empty());
    assert_eq!(result["total"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn search_pattern_keeps_error_for_broken_regex_intent() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("test.rs"), "fn main() {}\n").unwrap();
    // "(foo|bar" has unclosed group AND contains alternation — is_regex_like returns true
    let err = SearchPattern
        .call(json!({"pattern": "(foo|bar"}), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<RecoverableError>().is_some(),
        "broken regex with regex intent should be RecoverableError, got: {}",
        err
    );
}

#[tokio::test]
async fn search_pattern_valid_regex_has_no_mode() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(
        dir.path().join("test.rs"),
        "fn foo() {}\nfn bar() {}\n",
    )
    .unwrap();
    let result = SearchPattern
        .call(json!({"pattern": r"fn \w+"}), &ctx)
        .await
        .unwrap();
    assert!(result.get("mode").is_none());
}
```

- [ ] **Step 2: Update the existing `search_pattern_unescaped_paren_is_invalid_regex` test**

The existing test at line ~3850 uses `"if (name === '"` as an invalid regex. With the
new literal fallback, `is_regex_like("if (name === '")` returns `false` (the `(` has
no matching `)` in the string), so it will now trigger literal fallback instead of error.

Update the test to expect literal fallback:

```rust
#[tokio::test]
async fn search_pattern_unescaped_paren_literal_fallback() {
    // `if (name === '` — unescaped `(` makes invalid regex, but the input
    // doesn't look like intended regex, so it falls back to literal search.
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("api.js"),
        "function check() {\n  if (name === 'admin') { return true; }\n}\n",
    )
    .unwrap();

    let result = SearchPattern
        .call(
            json!({ "pattern": "if (name === '", "path": dir.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(
        result["mode"].as_str().unwrap(),
        "literal_fallback",
        "non-regex-looking invalid regex should use literal fallback"
    );
    // The literal text exists in the file — should find it
    assert!(
        !result["matches"].as_array().unwrap().is_empty(),
        "literal fallback should find the text"
    );
}
```

- [ ] **Step 3: Run tests to verify the new ones fail and the updated one fails**

Run: `cargo test -p codescout search_pattern_literal && cargo test -p codescout search_pattern_keeps && cargo test -p codescout search_pattern_valid_regex && cargo test -p codescout search_pattern_unescaped_paren`
Expected: compilation or assertion failures — no literal fallback logic exists yet

- [ ] **Step 4: Implement the literal fallback**

In `src/tools/file.rs`, replace the regex compilation block in `SearchPattern::call`.

**Before:**
```rust
let re = regex::RegexBuilder::new(pattern)
    .size_limit(1 << 20)
    .dfa_size_limit(1 << 20)
    .build()
    .map_err(|e| {
        RecoverableError::with_hint(
            format!("invalid regex: {e}"),
            "patterns are full regex syntax — escape metacharacters like \\( \\. \\[ for literals",
        )
    })?;
```

**After:**
```rust
let (re, is_literal_fallback) = match regex::RegexBuilder::new(pattern)
    .size_limit(1 << 20)
    .dfa_size_limit(1 << 20)
    .build()
{
    Ok(re) => (re, false),
    Err(e) => {
        if super::is_regex_like(pattern) {
            // User intended regex but it's broken — keep the error
            return Err(RecoverableError::with_hint(
                format!("invalid regex: {e}"),
                "patterns are full regex syntax — escape metacharacters like \\( \\. \\[ for literals",
            )
            .into());
        }
        // Plain text with metacharacters — search literally
        let escaped = regex::escape(pattern);
        let re = regex::RegexBuilder::new(&escaped)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
            .map_err(|e2| {
                RecoverableError::with_hint(
                    format!("invalid pattern even after escaping: {e2}"),
                    format!("original error: {e}"),
                )
            })?;
        (re, true)
    }
};
```

Then, at the end of the function, after `let mut result = json!({ "matches": matches, "total": shown_count });`, add:

```rust
if is_literal_fallback {
    result["mode"] = json!("literal_fallback");
    result["reason"] = json!("pattern was not valid regex — searched as literal text");
}
```

- [ ] **Step 5: Update `format_search_pattern` for compact rendering**

In `src/tools/file.rs`, in `format_search_pattern`, after the line
`let mut out = format!("{total} {match_word}\n");`, add:

```rust
if val.get("mode").and_then(|m| m.as_str()) == Some("literal_fallback") {
    out.insert_str(0, "[literal fallback] ");
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p codescout search_pattern_literal && cargo test -p codescout search_pattern_keeps && cargo test -p codescout search_pattern_valid_regex && cargo test -p codescout search_pattern_unescaped_paren`
Expected: all 5 tests PASS (4 new + 1 updated)

- [ ] **Step 7: Run full check**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 8: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: search_pattern falls back to literal search for non-regex text"
```

---
### Task 4: Prompt surface updates

**Files:**
- Modify: `src/prompts/server_instructions.md` — add anti-pattern row
- Modify: `src/prompts/onboarding_prompt.md` — add line to Step 6
- Modify: `src/tools/workflow.rs` — add line to Navigation Strategy

- [ ] **Step 1: Add anti-pattern row to server instructions**

In `src/prompts/server_instructions.md`, in the anti-patterns table (line ~53), add a new row after the last `|...|...|...|` row:

```
| `find_symbol(pattern="foo\|bar")` | `search_pattern(pattern="foo\|bar")` or separate `find_symbol` calls | `find_symbol` rejects regex-like patterns |
```

- [ ] **Step 2: Add guidance to onboarding prompt**

In `src/prompts/onboarding_prompt.md`, in the **Step 6: Code Exploration by Concept** section, after the paragraph ending with "it works without an index and still reveals how the codebase handles each concept.", add:

```
`find_symbol` searches by symbol name substring — use `search_pattern` for regex or
text discovery. Do not pass regex alternation (`foo|bar`) to `find_symbol`.
```

- [ ] **Step 3: Add guidance to workflow.rs prompt draft**

In `src/tools/workflow.rs`, in `build_system_prompt_draft()`, find the line:
```rust
draft.push_str("4. `find_symbol(\"Name\", include_body=true)` — read implementation\n");
```

Add after it:
```rust
draft.push_str("   - regex-like patterns belong in `search_pattern`, not `find_symbol`\n");
```

- [ ] **Step 4: Run full check**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/tools/workflow.rs
git commit -m "docs: update prompt surfaces for query-shape detection"
```

---

### Task 5: Final verification

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: compiles cleanly

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 3: Verify clippy is clean**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

- [ ] **Step 4: Verify the three prompt surfaces are consistent**

Search for `find_symbol` + `regex` across all three surfaces:
- `src/prompts/server_instructions.md` — has anti-pattern row
- `src/prompts/onboarding_prompt.md` — has Step 6 guidance
- `src/tools/workflow.rs` — has navigation rule

Run: `grep -n "regex" src/prompts/server_instructions.md src/prompts/onboarding_prompt.md` and check `workflow.rs` for the added line.

- [ ] **Step 5: Commit if any formatting was needed, then done**
