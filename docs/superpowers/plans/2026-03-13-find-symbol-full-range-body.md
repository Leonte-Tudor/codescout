# find_symbol Full-Range Body Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `find_symbol(include_body=true)` return the full symbol range (attributes + doc comments + declaration + body) so it matches what `replace_symbol` replaces — achieving read/write symmetry.

**Architecture:** Change `symbol_to_json` body extraction from `start_line` (selectionRange — fn keyword) to `editing_start_line` (range_start_line — includes attributes and doc comments). Add `body_start_line` field to JSON output so agents know where the body begins relative to `start_line`. No changes to `replace_symbol` — it already uses `editing_start_line`.

**Tech Stack:** Rust, LSP protocol, tree-sitter (fallback)

**Background — the bug this fixes:**
`find_symbol(include_body=true)` extracts body from `start_line` (the `fn` keyword). `replace_symbol` replaces from `editing_start_line` (includes `#[test]`, `/// doc`). When an agent reads a function, modifies it, and passes it back to `replace_symbol`, the attributes get consumed because the replacement range is wider than what was read. The original "Trust LSP" design spec (`docs/plans/complete/2026-03-02-symbol-range-redesign-design.md`) noted "both paths use the same range" as a goal but the implementation only changed the write path.

---

## Chunk 1: Core change + unit tests

### Task 1: Write failing test for body including attributes

**Files:**
- Modify: `src/tools/symbol.rs` (unit test section, near line ~4915 where `editing_start_line` tests live)

- [ ] **Step 1: Write the failing test**

Add a unit test that verifies `symbol_to_json` includes attributes in the body when `range_start_line` is set. Place it after the existing `editing_start_line` tests (around line 4968).

```rust
#[test]
fn symbol_to_json_body_includes_attributes_when_range_start_line_set() {
    let source = "#[test]\n/// A doc comment\nfn foo() {\n    body();\n}\n";
    let sym = SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: SymbolKind::Function,
        file: PathBuf::from("src/lib.rs"),
        start_line: 2, // fn keyword (0-indexed)
        end_line: 4,   // closing }
        start_col: 0,
        children: vec![],
        range_start_line: Some(0), // #[test] line
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "body should include #[test] attribute; got:\n{body}"
    );
    assert!(
        body.contains("/// A doc comment"),
        "body should include doc comment; got:\n{body}"
    );
    assert!(
        body.contains("fn foo()"),
        "body should include fn declaration; got:\n{body}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test symbol_to_json_body_includes_attributes -- --nocapture`
Expected: FAIL — body does not contain `#[test]` because `symbol_to_json` currently uses `start_line` (line 2), not `range_start_line` (line 0).

### Task 2: Write failing test for body_start_line field

**Files:**
- Modify: `src/tools/symbol.rs` (same test section)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn symbol_to_json_includes_body_start_line() {
    let source = "#[test]\nfn foo() {}\n";
    let sym = SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: SymbolKind::Function,
        file: PathBuf::from("src/lib.rs"),
        start_line: 1,
        end_line: 1,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    // body_start_line should be 1 (1-indexed, the #[test] line)
    assert_eq!(
        json["body_start_line"].as_u64(),
        Some(1),
        "body_start_line should be 1-indexed line where body begins (the attribute line)"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test symbol_to_json_includes_body_start_line -- --nocapture`
Expected: FAIL — `body_start_line` field does not exist.

### Task 3: Write failing test for None fallback path

**Files:**
- Modify: `src/tools/symbol.rs` (same test section)

- [ ] **Step 1: Write the failing test**

When `range_start_line` is `None`, the body should use `find_insert_before_line` to walk backward and include attributes.

```rust
#[test]
fn symbol_to_json_body_uses_heuristic_when_range_start_line_none() {
    let source = "#[test]\nfn foo() {\n    body();\n}\n";
    let sym = SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: SymbolKind::Function,
        file: PathBuf::from("src/lib.rs"),
        start_line: 1, // fn keyword
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: None, // tree-sitter / workspace/symbol path
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "body should include #[test] via heuristic fallback; got:\n{body}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test symbol_to_json_body_uses_heuristic -- --nocapture`
Expected: FAIL — body currently starts from `start_line` (line 1), missing `#[test]` on line 0.

### Task 4: Implement the fix in symbol_to_json

**Files:**
- Modify: `src/tools/symbol.rs` — function `symbol_to_json` (line ~197)

- [ ] **Step 1: Implement the change**

In `symbol_to_json`, replace the body extraction block:

```rust
// BEFORE (lines ~224-228):
if include_body {
    if let Some(src) = source_code {
        let lines: Vec<&str> = src.lines().collect();
        let start = sym.start_line as usize;
        let end = (sym.end_line as usize + 1).min(lines.len());
        if start < lines.len() {
            map.insert("body".into(), json!(lines[start..end].join("\n")));
        }
    }
}

// AFTER:
if include_body {
    if let Some(src) = source_code {
        let lines: Vec<&str> = src.lines().collect();
        let body_start = editing_start_line(sym, &lines);
        let end = (sym.end_line as usize + 1).min(lines.len());
        if body_start < lines.len() {
            map.insert("body".into(), json!(lines[body_start..end].join("\n")));
            // 1-indexed line where body starts (may differ from start_line
            // when attributes or doc comments precede the declaration)
            map.insert("body_start_line".into(), json!(body_start + 1));
        }
    }
}
```

Note: `body_start_line` is only emitted when `include_body` is true and a body is present. It is 1-indexed (matching `start_line` and `end_line` in the output). When there are no attributes, `body_start_line == start_line`.

- [ ] **Step 2: Run the three new tests**

Run: `cargo test symbol_to_json_body_includes_attributes symbol_to_json_includes_body_start_line symbol_to_json_body_uses_heuristic -- --nocapture`
Expected: All 3 PASS.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass. Some existing tests that assert exact body content may need updating (check in Task 5).

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean.

### Task 5: Fix any broken existing tests

**Files:**
- Modify: `src/tools/symbol.rs` (unit tests) and `tests/symbol_lsp.rs` (integration tests)

Existing tests that assert body content may now include attribute/doc lines they didn't before. Only tests using `sym_with_range` or fixtures with attributes above symbols would be affected.

- [ ] **Step 1: Run full test suite and identify failures**

Run: `cargo test 2>&1 | grep FAILED`
Expected: Note any test names that fail.

- [ ] **Step 2: Update failing tests to expect the new body content**

For each failing test, the body now starts from `range_start_line` (or heuristic). Update assertions to expect the full range including attributes/doc comments.

- [ ] **Step 3: Run full suite again**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/tools/symbol.rs tests/symbol_lsp.rs
git commit -m "fix(find_symbol): include attributes and doc comments in body for read/write symmetry

find_symbol(include_body=true) now extracts body from editing_start_line
(range_start_line with heuristic fallback) instead of start_line. This
matches what replace_symbol replaces — agents see exactly the range they
will be replacing. Adds body_start_line to JSON output."
```

---

## Chunk 2: Integration test with replace_symbol round-trip

### Task 6: Write integration test — read/replace round-trip preserves attributes

**Files:**
- Modify: `tests/symbol_lsp.rs`

This is the critical test: read a function with `find_symbol`, modify the body, pass it back to `replace_symbol`, verify attributes are preserved.

- [ ] **Step 1: Write the failing test**

```rust
/// Round-trip: find_symbol(include_body) → modify → replace_symbol preserves attributes.
/// This is the bug that motivated the full-range body change.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_attributes() {
    // File layout (0-indexed):
    //  0: "#[test]"                     <- range_start = 0
    //  1: "/// A test function"
    //  2: "fn target() {"               <- selectionRange.start = 2
    //  3: "    old_body();"
    //  4: "}"                           <- end = 4
    let src = "#[test]\n/// A test function\nfn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(
            file.clone(),
            vec![sym_with_range("target", 2, 4, 0, file)],
        )
    })
    .await;

    // Step 1: Read the symbol body (simulates what the agent does)
    let find_result = FindSymbol
        .call(
            json!({
                "name_path": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    // The body should include #[test] and /// doc
    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "find_symbol body should include attribute; got:\n{body}"
    );

    // Step 2: Agent modifies the body (changes old_body to new_body, keeps attrs)
    let new_body = body.replace("old_body()", "new_body()");

    // Step 3: Replace with the modified body
    ReplaceSymbol
        .call(
            json!({
                "path": "src/lib.rs",
                "name_path": "target",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("#[test]"),
        "attribute must be preserved after round-trip; got:\n{result}"
    );
    assert!(
        result.contains("/// A test function"),
        "doc comment must be preserved after round-trip; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
}
```

- [ ] **Step 2: Run test**

Run: `cargo test replace_symbol_round_trip_preserves_attributes -- --nocapture`
Expected: PASS (if Task 4 was implemented correctly, this should pass without additional changes).

### Task 7: Write integration test — Python decorator round-trip

**Files:**
- Modify: `tests/symbol_lsp.rs`

- [ ] **Step 1: Write the test**

```rust
/// Python: decorators above def are in range_start, docstrings are inside the body.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_python_decorator() {
    // File layout (0-indexed):
    //  0: "@staticmethod"               <- range_start = 0
    //  1: "def target():"               <- selectionRange.start = 1
    //  2: "    old_body()"              <- end = 2
    let src = "@staticmethod\ndef target():\n    old_body()\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.py", src)], |root| {
        let file = root.join("src/lib.py");
        MockLspClient::new().with_symbols(
            file.clone(),
            vec![sym_with_range("target", 1, 2, 0, file)],
        )
    })
    .await;

    let find_result = FindSymbol
        .call(
            json!({
                "name_path": "target",
                "path": "src/lib.py",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("@staticmethod"),
        "body should include decorator; got:\n{body}"
    );

    let new_body = body.replace("old_body()", "new_body()");

    ReplaceSymbol
        .call(
            json!({
                "path": "src/lib.py",
                "name_path": "target",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.py")).unwrap();
    assert!(
        result.contains("@staticmethod"),
        "decorator must survive round-trip; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
}
```

- [ ] **Step 2: Run test**

Run: `cargo test replace_symbol_round_trip_preserves_python_decorator -- --nocapture`
Expected: PASS.

### Task 8: Write integration test — Java annotation round-trip

**Files:**
- Modify: `tests/symbol_lsp.rs`

- [ ] **Step 1: Write the test**

```rust
/// Java: @Override annotation + Javadoc above method.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_java_annotation() {
    // File layout (0-indexed):
    //  0: "/** Javadoc comment */"       <- range_start = 0
    //  1: "@Override"
    //  2: "public void target() {"       <- selectionRange.start = 2
    //  3: "    oldBody();"
    //  4: "}"                            <- end = 4
    let src = "/** Javadoc comment */\n@Override\npublic void target() {\n    oldBody();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/Main.java", src)], |root| {
        let file = root.join("src/Main.java");
        MockLspClient::new().with_symbols(
            file.clone(),
            vec![sym_with_range("target", 2, 4, 0, file)],
        )
    })
    .await;

    let find_result = FindSymbol
        .call(
            json!({
                "name_path": "target",
                "path": "src/Main.java",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(body.contains("@Override"), "body should include annotation; got:\n{body}");
    assert!(body.contains("/** Javadoc"), "body should include Javadoc; got:\n{body}");

    let new_body = body.replace("oldBody()", "newBody()");

    ReplaceSymbol
        .call(
            json!({
                "path": "src/Main.java",
                "name_path": "target",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/Main.java")).unwrap();
    assert!(result.contains("@Override"), "annotation must survive; got:\n{result}");
    assert!(result.contains("/** Javadoc"), "Javadoc must survive; got:\n{result}");
    assert!(result.contains("newBody()"), "new body applied; got:\n{result}");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test replace_symbol_round_trip_preserves_java_annotation -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Run full suite, clippy, fmt**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All clean.

- [ ] **Step 4: Commit**

```bash
git add tests/symbol_lsp.rs
git commit -m "test: add round-trip integration tests for find_symbol + replace_symbol

Verify read/write symmetry for Rust (#[test] + ///doc), Python
(@decorator), and Java (@Override + Javadoc). All three confirm that
attributes survive the find→modify→replace round-trip."
```

---

## Chunk 3: Documentation and tool description updates

### Task 9: Update replace_symbol tool description

**Files:**
- Modify: `src/tools/symbol.rs` — `impl Tool for ReplaceSymbol` description method

- [ ] **Step 1: Update the description**

The description should clarify that `new_body` should include the full declaration including attributes and doc comments — matching what `find_symbol(include_body=true)` returns.

Find the `description()` method for `ReplaceSymbol` and update it to mention that `new_body` should contain the full symbol including attributes and doc comments.

- [ ] **Step 2: Verify no tests break**

Run: `cargo test`
Expected: All pass (description change is metadata only).

### Task 10: Update server_instructions.md

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Update the tool reference table**

In the "By task" table, update the `replace_symbol` row to note that `new_body` includes attributes. In the `### Symbol Editing (LSP)` section, update the `replace_symbol` entry.

- [ ] **Step 2: Verify build compiles (instructions are baked in via include_str!)**

Run: `cargo build`
Expected: Compiles.

### Task 11: Log in tool misbehaviors doc

**Files:**
- Modify: `docs/TODO-tool-misbehaviors.md`

- [ ] **Step 1: Add BUG entry for the asymmetry**

Add a new entry documenting the read/write asymmetry bug and its fix. Mark it as ✅ FIXED. Use the next available BUG number.

### Task 12: Final verification and commit

- [ ] **Step 1: Full check**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All clean, all tests pass.

- [ ] **Step 2: Commit**

```bash
git add src/tools/symbol.rs src/prompts/server_instructions.md docs/TODO-tool-misbehaviors.md
git commit -m "docs: update replace_symbol description and instructions for full-range body

Document that new_body should include attributes and doc comments,
matching find_symbol(include_body=true) output. Log BUG fix."
```

- [ ] **Step 3: Build release binary for live testing**

Run: `cargo build --release`
Then restart MCP with `/mcp` and manually verify:
1. `find_symbol(name_path="some_test_fn", include_body=true)` includes `#[test]` in body
2. `body_start_line` field is present
3. Round-trip: read → modify → replace preserves attributes
