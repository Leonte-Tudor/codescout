# find_symbol body fallback — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** When `find_symbol(include_body=true)` hits a degenerate `workspace/symbol` range, transparently fall back to `document_symbols` for that file instead of erroring.

**Architecture:** Add a `resolve_range_via_document_symbols` async helper that queries `textDocument/documentSymbol` for a single file and matches the symbol by name+line. The workspace/symbol loop in `FindSymbol::call()` catches `validate_symbol_range` errors and attempts this fallback before propagating the error.

**Tech Stack:** Rust, async_trait, LSP (workspace/symbol + textDocument/documentSymbol), MockLspClient for testing.

---

### Task 1: Extend MockLspClient with workspace_symbols support

**Files:**
- Modify: `src/lsp/mock.rs:12-15` (struct fields)
- Modify: `src/lsp/mock.rs:18-23` (new constructor)
- Modify: `src/lsp/mock.rs:63-65` (workspace_symbols impl)

**Step 1: Add `workspace_results` field to MockLspClient**

Add a new field after `definitions`:

```rust
pub struct MockLspClient {
    symbols: HashMap<PathBuf, Vec<SymbolInfo>>,
    definitions: HashMap<(u32, u32), Vec<lsp_types::Location>>,
    workspace_results: Vec<SymbolInfo>,
}
```

**Step 2: Update `new()` to initialize the field**

```rust
pub fn new() -> Self {
    Self {
        symbols: HashMap::new(),
        definitions: HashMap::new(),
        workspace_results: vec![],
    }
}
```

**Step 3: Add `with_workspace_symbols` builder method**

Insert after `with_definitions`:

```rust
pub fn with_workspace_symbols(mut self, syms: Vec<SymbolInfo>) -> Self {
    self.workspace_results = syms;
    self
}
```

**Step 4: Update `workspace_symbols` to return stored data**

```rust
async fn workspace_symbols(&self, _query: &str) -> anyhow::Result<Vec<SymbolInfo>> {
    Ok(self.workspace_results.clone())
}
```

**Step 5: Run tests to verify no regressions**

Run: `cargo test -p code-explorer -- mock`
Expected: All existing tests pass (workspace_symbols previously returned empty, existing tests don't depend on it returning data).

**Step 6: Commit**

```
git add src/lsp/mock.rs
git commit -m "refactor(mock): add workspace_symbols support to MockLspClient"
```

---

### Task 2: Write failing test for the fallback

**Files:**
- Modify: `src/tools/symbol.rs` (add test at the end of the `#[cfg(test)]` module)

**Step 1: Write the failing test**

Add this test near the existing `validate_symbol_range_*` tests (after line ~2903):

```rust
#[tokio::test]
async fn find_symbol_falls_back_to_document_symbols_on_bad_workspace_range() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider, SymbolInfo, SymbolKind};

    let dir = tempfile::tempdir().unwrap();
    // Create a Rust source file with a multi-line function
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(
        &file,
        "fn helper(x: i32) -> i32 {\n    let y = x + 1;\n    y * 2\n}\n",
    )
    .unwrap();

    // workspace/symbol returns degenerate range (start == end)
    let degenerate = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0, // degenerate — only the name line
        start_col: 3,
        children: vec![],
        detail: None,
    };

    // document_symbols returns correct range
    let correct = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 3,
        start_col: 3,
        children: vec![],
        detail: None,
    };

    let mock = MockLspClient::new()
        .with_workspace_symbols(vec![degenerate])
        .with_symbols(&file, vec![correct]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = crate::agent::Agent::new_with_project(dir.path().to_path_buf());
    let output_buffer = std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new());
    let ctx = crate::tools::ToolContext {
        project_root: dir.path().to_path_buf(),
        config: std::sync::Arc::new(crate::config::ProjectConfig::default()),
        lsp: lsp,
        agent: std::sync::Arc::new(agent),
        output_buffer,
    };

    let result = FindSymbol
        .call(
            serde_json::json!({
                "pattern": "helper",
                "include_body": true,
            }),
            &ctx,
        )
        .await;

    // Should succeed — fallback to document_symbols resolves the correct range
    let val = result.expect("find_symbol should recover via document_symbols fallback");
    let symbols = val["symbols"].as_array().expect("symbols array");
    assert_eq!(symbols.len(), 1, "should find exactly one symbol");

    let body = symbols[0]["body"]
        .as_str()
        .expect("body should be present");
    assert!(
        body.contains("let y = x + 1"),
        "body should contain function contents; got: {body}"
    );
}
```

**Step 2: Run the test to verify it fails**

Run: `cargo test find_symbol_falls_back_to_document_symbols -- --nocapture`
Expected: FAIL — `validate_symbol_range` returns `RecoverableError` for the degenerate range, and there's no fallback yet.

---

### Task 3: Implement `resolve_range_via_document_symbols`

**Files:**
- Modify: `src/tools/symbol.rs` — add new function after `find_ast_end_line_in` (after line 283)

**Step 1: Write the fallback function**

Insert after `find_ast_end_line_in` (line 283):

```rust
/// When `workspace/symbol` returns a degenerate range, attempt to resolve the
/// correct range by querying `textDocument/documentSymbol` for the symbol's file.
/// Returns the corrected SymbolInfo if found, None otherwise.
async fn resolve_range_via_document_symbols(
    sym: &SymbolInfo,
    ctx: &ToolContext,
) -> Option<SymbolInfo> {
    let lang = crate::ast::detect_language(&sym.file)?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    let root = ctx.agent.require_project_root().await.ok()?;
    let client = ctx.lsp.get_or_start(lang, &root).await.ok()?;
    let doc_symbols = client.document_symbols(&sym.file, language_id).await.ok()?;
    find_matching_symbol(&doc_symbols, &sym.name, sym.start_line)
}

/// Recursively search a document symbol tree for a symbol matching `name`
/// within ±1 line of `lsp_start`. Returns a clone of the matching SymbolInfo.
fn find_matching_symbol(symbols: &[SymbolInfo], name: &str, lsp_start: u32) -> Option<SymbolInfo> {
    for sym in symbols {
        if sym.name == name && sym.start_line.abs_diff(lsp_start) <= 1 {
            return Some(sym.clone());
        }
        if let Some(found) = find_matching_symbol(&sym.children, name, lsp_start) {
            return Some(found);
        }
    }
    None
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles (function exists but isn't called yet).

---

### Task 4: Wire up the fallback in FindSymbol::call()

**Files:**
- Modify: `src/tools/symbol.rs:742-762` (workspace/symbol loop)

**Step 1: Replace the hard error with fallback logic**

Find this block (around lines 742-762):

```rust
                    if name_ok && kind_ok {
                        // Validate range but don't silently fix — if degenerate, the agent
                        // sees the error and can fall back to edit_file.
                        if include_body {
                            validate_symbol_range(&sym)?;
                        }
                        let source = if include_body {
                            std::fs::read_to_string(&sym.file).ok()
                        } else {
                            None
                        };
                        matches.push(symbol_to_json(
                            &sym,
                            include_body,
                            source.as_deref(),
                            depth,
                            true,
                        ));
                    }
```

Replace with:

```rust
                    if name_ok && kind_ok {
                        // When include_body is requested, validate the range. If
                        // workspace/symbol returned a degenerate range, fall back to
                        // document_symbols for the file to get the correct range.
                        let sym = if include_body {
                            match validate_symbol_range(&sym) {
                                Ok(()) => sym,
                                Err(_) => {
                                    match resolve_range_via_document_symbols(&sym, ctx).await {
                                        Some(resolved) => resolved,
                                        None => {
                                            // document_symbols fallback failed too — propagate
                                            validate_symbol_range(&sym)?;
                                            unreachable!()
                                        }
                                    }
                                }
                            }
                        } else {
                            sym
                        };
                        let source = if include_body {
                            std::fs::read_to_string(&sym.file).ok()
                        } else {
                            None
                        };
                        matches.push(symbol_to_json(
                            &sym,
                            include_body,
                            source.as_deref(),
                            depth,
                            true,
                        ));
                    }
```

**Step 2: Run the new test to verify it passes**

Run: `cargo test find_symbol_falls_back_to_document_symbols -- --nocapture`
Expected: PASS — the fallback resolves the correct range and extracts the body.

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

**Step 5: Run fmt**

Run: `cargo fmt`

**Step 6: Commit**

```
git add src/tools/symbol.rs src/lsp/mock.rs
git commit -m "fix(find_symbol): fall back to document_symbols on bad workspace/symbol range

When workspace/symbol returns a degenerate range (start==end) for a
symbol, find_symbol(include_body=true) now transparently queries
document_symbols for the affected file to resolve the correct range,
instead of returning a RecoverableError.

Write tools (replace_symbol, insert_code, remove_symbol) keep the hard
error since they already use document_symbols."
```
