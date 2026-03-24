# edit_file Hard Block Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the soft-block (pending_ack + acknowledge_risk bypass) on multi-line source edits with a hard RecoverableError for structural definitions on LSP-supported languages.

**Architecture:** The gate in `EditFile::call()` checks 4 conditions (multi-line, source path, LSP available, def keyword present). All must be true to block. Non-LSP languages and non-structural edits pass through freely. All bypass infrastructure (PendingAckEdit, @ack_* handles, acknowledge_risk) is removed.

**Tech Stack:** Rust, codescout MCP tools for code navigation and editing.

**Spec:** `docs/superpowers/specs/2026-03-15-edit-file-hard-block-design.md`

---

## Chunk 1: Infrastructure — Add `has_lsp_config`, helpers, remove `"type "`

### Task 1: Add `has_lsp_config()` to LSP servers module

**Files:**
- Modify: `src/lsp/servers/mod.rs` (after `lsp_language_id` function, ~line 93)

- [ ] **Step 1: Write the test**

Add a test to `src/lsp/servers/mod.rs` (in a `#[cfg(test)]` module if one exists, otherwise create one):

```rust
#[test]
fn has_lsp_config_covers_all_configured_languages() {
    // Tier 1: Full support (tree-sitter + LSP)
    assert!(has_lsp_config("rust"));
    assert!(has_lsp_config("python"));
    assert!(has_lsp_config("typescript"));
    assert!(has_lsp_config("javascript"));
    assert!(has_lsp_config("tsx"));
    assert!(has_lsp_config("jsx"));
    assert!(has_lsp_config("go"));
    assert!(has_lsp_config("java"));
    assert!(has_lsp_config("kotlin"));

    // Tier 2: LSP-only (no tree-sitter grammars)
    assert!(has_lsp_config("c"));
    assert!(has_lsp_config("cpp"));
    assert!(has_lsp_config("csharp"));
    assert!(has_lsp_config("ruby"));

    // Tier 3: No LSP
    assert!(!has_lsp_config("php"));
    assert!(!has_lsp_config("swift"));
    assert!(!has_lsp_config("scala"));
    assert!(!has_lsp_config("elixir"));
    assert!(!has_lsp_config("haskell"));
    assert!(!has_lsp_config("lua"));
    assert!(!has_lsp_config("bash"));
    assert!(!has_lsp_config("markdown"));
    assert!(!has_lsp_config("unknown"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test has_lsp_config_covers_all -- --nocapture`
Expected: FAIL — `has_lsp_config` not found.

- [ ] **Step 3: Implement `has_lsp_config`**

Add to `src/lsp/servers/mod.rs` after `lsp_language_id()`:

```rust
/// Returns true if we have a default LSP server config for this language.
/// Used by `edit_file` to decide whether symbol tools are a viable alternative.
pub fn has_lsp_config(lang: &str) -> bool {
    matches!(
        lang,
        "rust"
            | "python"
            | "typescript"
            | "javascript"
            | "tsx"
            | "jsx"
            | "go"
            | "java"
            | "kotlin"
            | "c"
            | "cpp"
            | "csharp"
            | "ruby"
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test has_lsp_config_covers_all -- --nocapture`
Expected: PASS

### Task 2: Add helpers and remove `"type "` from DEF_KEYWORDS in `file.rs`

**Files:**
- Modify: `src/tools/file.rs` (~lines 1376-1440)

- [ ] **Step 1: Write tests for helpers**

Add to the `#[cfg(test)]` module in `src/tools/file.rs`:

```rust
#[test]
fn contains_def_keyword_detects_definitions() {
    assert!(contains_def_keyword("fn foo() {\n    body\n}"));
    assert!(contains_def_keyword("async fn bar()"));
    assert!(contains_def_keyword("def greet():\n    pass"));
    assert!(contains_def_keyword("class Foo {\n}"));
    assert!(contains_def_keyword("struct Bar {\n}"));
    assert!(contains_def_keyword("impl Foo {\n}"));
    assert!(contains_def_keyword("trait MyTrait {\n}"));
    assert!(contains_def_keyword("interface Iface {\n}"));
    assert!(contains_def_keyword("enum Color {\n}"));
    assert!(contains_def_keyword("func main() {\n}"));
    assert!(contains_def_keyword("fun doThing() {\n}"));
    assert!(contains_def_keyword("function handler() {\n}"));
    assert!(contains_def_keyword("async function handler() {\n}"));
    assert!(contains_def_keyword("async def handler():\n    pass"));
}

#[test]
fn contains_def_keyword_rejects_non_definitions() {
    assert!(!contains_def_keyword("use {Foo,\n    Bar}"));
    assert!(!contains_def_keyword("import {a,\n    b}"));
    assert!(!contains_def_keyword("let x = 1;\nlet y = 2;"));
    assert!(!contains_def_keyword("// this is a comment\n// another line"));
    assert!(!contains_def_keyword("\"some string\nwith newlines\""));
    // "type " was removed — too many false positives
    assert!(!contains_def_keyword("type Foo = Bar;\ntype Baz = Qux;"));
}

#[test]
fn has_lsp_support_by_path() {
    assert!(has_lsp_support("src/main.rs"));
    assert!(has_lsp_support("lib.py"));
    assert!(has_lsp_support("index.ts"));
    assert!(has_lsp_support("App.java"));
    assert!(has_lsp_support("main.go"));
    assert!(has_lsp_support("file.c"));
    assert!(has_lsp_support("file.cpp"));
    assert!(has_lsp_support("file.rb"));
    // No LSP languages
    assert!(!has_lsp_support("script.lua"));
    assert!(!has_lsp_support("script.sh"));
    assert!(!has_lsp_support("app.swift"));
    assert!(!has_lsp_support("app.ex"));
    assert!(!has_lsp_support("README.md"));
    assert!(!has_lsp_support("config.toml"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test contains_def_keyword_detects -- --nocapture && cargo test has_lsp_support_by_path -- --nocapture`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement helpers and remove `"type "` from DEF_KEYWORDS**

In `src/tools/file.rs`, modify `DEF_KEYWORDS` to remove `"type "`:

```rust
const DEF_KEYWORDS: &[&str] = &[
    "fn ",
    "def ",
    "func ",
    "fun ",
    "function ",
    "async fn ",
    "async def ",
    "async function ",
    "class ",
    "struct ",
    "impl ",
    "trait ",
    "interface ",
    "enum ",
];
```

Add these two helpers (near `DEF_KEYWORDS`):

```rust
/// Returns true if the string contains a definition keyword from DEF_KEYWORDS.
fn contains_def_keyword(s: &str) -> bool {
    DEF_KEYWORDS.iter().any(|kw| s.contains(kw))
}

/// Returns true if the file's language has LSP support (symbol tools can work).
fn has_lsp_support(path: &str) -> bool {
    let p = std::path::Path::new(path);
    crate::ast::detect_language(p)
        .map(|lang| crate::lsp::servers::has_lsp_config(lang))
        .unwrap_or(false)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test contains_def_keyword -- --nocapture && cargo test has_lsp_support_by_path -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lsp/servers/mod.rs src/tools/file.rs
git commit -m "feat(edit_file): add has_lsp_config, contains_def_keyword, has_lsp_support helpers; remove 'type ' from DEF_KEYWORDS"
```

---

## Chunk 2: Replace the gate + simplify `infer_edit_hint`

### Task 3: Replace soft block with hard RecoverableError

**Files:**
- Modify: `src/tools/file.rs` (~lines 1470-1545)

- [ ] **Step 1: Write the failing tests for the new gate**

Add to `#[cfg(test)]` in `src/tools/file.rs`:

```rust
#[tokio::test]
async fn edit_file_blocks_def_keyword_on_lsp_language() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "fn foo() {\n    old();\n}\n").unwrap();

    let result = EditFile
        .call(
            json!({
                "path": "src/lib.rs",
                "old_string": "fn foo() {\n    old();\n}",
                "new_string": "fn foo() {\n    new();\n}"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err(), "should hard-block structural edit on LSP language");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("symbol definition"),
        "error should mention symbol definition, got: {err}"
    );
}

#[tokio::test]
async fn edit_file_passes_non_lsp_language() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("script.lua");
    std::fs::write(&path, "function greet()\n    print('hi')\nend\n").unwrap();

    let result = EditFile
        .call(
            json!({
                "path": "script.lua",
                "old_string": "function greet()\n    print('hi')\nend",
                "new_string": "function greet()\n    print('hello')\nend"
            }),
            &ctx,
        )
        .await;

    assert!(
        result.is_ok(),
        "should allow structural edit on non-LSP language: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn edit_file_passes_no_def_keyword() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "use crate::{\n    Foo,\n    Bar,\n};\n").unwrap();

    let result = EditFile
        .call(
            json!({
                "path": "src/lib.rs",
                "old_string": "use crate::{\n    Foo,\n    Bar,\n}",
                "new_string": "use crate::{\n    Foo,\n    Bar,\n    Baz,\n}"
            }),
            &ctx,
        )
        .await;

    assert!(
        result.is_ok(),
        "should allow import list edit (no def keyword): {:?}",
        result.err()
    );
}

#[tokio::test]
async fn edit_file_passes_multiline_non_source() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("README.md");
    std::fs::write(&path, "line one\nline two\n").unwrap();

    let result = EditFile
        .call(
            json!({
                "path": "README.md",
                "old_string": "line one\nline two",
                "new_string": "updated one\nupdated two"
            }),
            &ctx,
        )
        .await;

    assert!(
        result.is_ok(),
        "should allow multi-line edit on non-source file: {:?}",
        result.err()
    );
}
```

- [ ] **Step 2: Run new tests to verify they fail**

Run: `cargo test edit_file_blocks_def_keyword -- --nocapture`
Expected: FAIL — currently returns `Ok` with `pending_ack`, not `Err`.

- [ ] **Step 3: Replace the gate logic in `EditFile::call()`**

In `src/tools/file.rs`, find the `EditFile::call()` method. Make these changes:

**a) Remove** the `acknowledge_risk` parsing (~line 1473):
```rust
// DELETE: let acknowledge_risk = parse_bool_param(&input["acknowledge_risk"]);
```

**b) Remove** the `@ack_*` handle dispatch block (~lines 1476-1488):
```rust
// DELETE the entire block:
// if path.starts_with("@ack_") { ... }
```

**c) Replace** the soft block (~lines 1528-1544) with:
```rust
// Hard-block multi-line edits that contain definition keywords on LSP-supported languages.
if old_string.contains('\n')
    && crate::util::path_security::is_source_path(path)
    && has_lsp_support(path)
    && contains_def_keyword(old_string)
{
    let hint = infer_edit_hint(old_string, new_string);
    let keyword = DEF_KEYWORDS
        .iter()
        .find(|kw| old_string.contains(*kw))
        .unwrap_or(&"");
    return Err(super::RecoverableError::with_hint(
        format!(
            "multi-line edit contains a symbol definition ({keyword:?}) \
             — use symbol tools for structural changes"
        ),
        hint,
    )
    .into());
}
```

- [ ] **Step 4: Run new tests to verify they pass**

Run: `cargo test edit_file_blocks_def_keyword -- --nocapture && cargo test edit_file_passes_non_lsp -- --nocapture && cargo test edit_file_passes_no_def_keyword -- --nocapture`
Expected: PASS

### Task 4: Simplify `infer_edit_hint`

**Files:**
- Modify: `src/tools/file.rs` (~lines 1394-1440)

- [ ] **Step 1: Simplify `infer_edit_hint()`**

Replace the function body. Remove both `looks_like_import` blocks and all `acknowledge_risk` references. The function is now only called after the gate confirms a def keyword is present, so the DEF_KEYWORDS check is always true — simplify to a clean three-way branch:

```rust
/// Suggests the right symbol tool when `edit_file` blocks a structural source edit.
/// Called only after the gate confirms a definition keyword is present.
fn infer_edit_hint(old_string: &str, new_string: &str) -> &'static str {
    if new_string.is_empty() {
        return "remove_symbol(name_path, path) — deletes the symbol and its doc comments/attributes";
    }
    if new_string.len() > old_string.len() {
        return "insert_code(name_path, path, code, position) — inserts before or after a named symbol";
    }
    "replace_symbol(name_path, path, new_body) — replaces the symbol body via LSP"
}
```

- [ ] **Step 2: Run full test suite to check for regressions**

Run: `cargo test -- --nocapture 2>&1 | tail -30`
Expected: Some old tests will fail (the ones we'll remove in Task 5). New tests should pass.

- [ ] **Step 3: Commit (will finalize after test cleanup in Task 5)**

Do not commit yet — old tests still reference removed infrastructure. Continue to Task 5.

### Task 5: Remove old tests, update existing tests

**Files:**
- Modify: `src/tools/file.rs` (test module, ~lines 3856-4200)

- [ ] **Step 1: Remove old tests**

Remove these test functions entirely from `src/tools/file.rs`:
- `infer_edit_hint_import_list_suggests_acknowledge_risk` (~line 3856)
- `edit_file_warns_multiline_on_rust_source` (~line 3897) — replaced by new `edit_file_blocks_def_keyword_on_lsp_language` (already added in Task 3)
- `edit_file_blocking_hint_always_includes_acknowledge_risk` (~line 3929)
- `edit_file_import_list_hint_suggests_acknowledge_risk_not_insert_code` (~line 3958)
- `edit_file_ack_handle_executes_edit` (~line 4146)
- `edit_file_acknowledge_risk_bypasses_source_check` (~line 4184)

Also update these two tests that assert `pending_ack` — change them to assert `is_err()` with `"symbol definition"` in the error message (same pattern as the new `edit_file_blocks_def_keyword_on_lsp_language` test):
- `edit_file_warns_multiline_python` (~line 4036) — writes `def greet():\n    print('hello')` to `.py` file
- `edit_file_warns_hint_suggests_remove_when_new_empty` (~line 4063) — writes `fn foo()` to `.rs` file with empty `new_string`, change assertion to expect `is_err()` with `"remove_symbol"` in the error

- [ ] **Step 2: Run the full test suite**

Run: `cargo test -- --nocapture 2>&1 | tail -30`
Expected: All tests in `file.rs` pass. Some tests in `workflow.rs` may still fail (Task 7).

- [ ] **Step 3: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat(edit_file): replace soft block with hard RecoverableError for structural edits

Multi-line edits containing definition keywords (fn, def, class, struct, etc.)
on LSP-supported languages now return a RecoverableError instead of a bypassable
pending_ack. No acknowledge_risk, no @ack_* handles, no bypass.

Languages without LSP (lua, bash, php, etc.) and non-structural edits (imports,
comments, strings) pass through freely."
```

---

## Chunk 3: Remove pending-edit infrastructure + cleanup

### Task 6: Remove PendingAckEdit from OutputBuffer

**Files:**
- Modify: `src/tools/output_buffer.rs` (~lines 37-44, 63-66, 82-84, 332-373)

- [ ] **Step 1: Remove `PendingAckEdit` struct** (~lines 37-44)

Delete the struct and its doc comment:
```rust
// DELETE:
// /// A multi-line source edit held pending agent acknowledgment.
// #[derive(Debug, Clone)]
// pub struct PendingAckEdit {
//     pub path: String,
//     pub old_string: String,
//     pub new_string: String,
//     pub replace_all: bool,
// }
```

- [ ] **Step 2: Remove pending-edit fields from `OutputBufferInner`**

Remove from the struct (~lines 63-66):
```rust
// DELETE:
// // --- pending-ack store (source edits) ---
// pending_edits: HashMap<String, PendingAckEdit>,
// pending_edits_order: Vec<String>,
// max_pending: usize,
```

And remove the corresponding initialization in `OutputBuffer::new()` (~lines 82-84):
```rust
// DELETE:
// pending_edits: HashMap::new(),
// pending_edits_order: Vec::new(),
// max_pending: 20,
```

- [ ] **Step 3: Remove `store_pending_edit()` method** (~lines 332-365)

Delete the entire method and its doc comment.

- [ ] **Step 4: Remove `get_pending_edit()` method** (~lines 369-373)

Delete the entire method and its doc comment.

- [ ] **Step 5: Verify compilation**

Run: `cargo build 2>&1 | tail -20`
Expected: May show unused-import warnings but should compile. If `PendingAckEdit` is used elsewhere, the compiler will tell us.

### Task 7: Remove cross-tool guard in workflow.rs

**Files:**
- Modify: `src/tools/workflow.rs` (~lines 1482-1490, ~line 4024)

- [ ] **Step 1: Remove the edit-ack cross-tool guard**

In `run_command`'s `call()` method, find the block around line 1482:
```rust
// DELETE this block:
// if looks_like_ack_handle(command) {
//     // Cross-tool guard: edit_file also issues @ack_ handles (pending_edits store).
//     if ctx.output_buffer.get_pending_edit(command).is_some() {
//         return Err(super::RecoverableError::with_hint(
//             "this ack handle belongs to edit_file, not run_command",
//             ...
//         ).into());
//     }
// }
```

Note: Only remove the `get_pending_edit` inner check, NOT the entire `looks_like_ack_handle` block — `run_command` has its own ack mechanism for dangerous commands that uses a separate `pending_acks` store.

- [ ] **Step 2: Remove the test**

Delete `run_command_rejects_edit_file_ack_handle_with_clear_error` (~line 4024).

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo build && cargo test -- --nocapture 2>&1 | tail -30`
Expected: PASS — no remaining references to `PendingAckEdit` or `get_pending_edit`.

- [ ] **Step 4: Commit**

```bash
git add src/tools/output_buffer.rs src/tools/workflow.rs
git commit -m "refactor: remove PendingAckEdit infrastructure from OutputBuffer

Removes PendingAckEdit struct, store_pending_edit/get_pending_edit methods,
pending_edits fields, and the cross-tool guard in run_command. This dead code
was part of the soft-block bypass mechanism replaced by hard RecoverableError."
```

---

## Chunk 4: Update prompts + final verification

### Task 8: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md` (~lines 17-19, 78, 111)

- [ ] **Step 1: Update Iron Law #2** (~line 17)

Change from:
```
2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `replace_symbol`, `insert_code`,
   `remove_symbol`, or `rename_symbol`. `edit_file` is for imports, literals, comments, config.
   Multi-line edits on source files are blocked — the tool tells you which symbol tool to use.
```

To:
```
2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `replace_symbol`, `insert_code`,
   `remove_symbol`, or `rename_symbol`. `edit_file` is for imports, literals, comments, config.
   Multi-line edits containing definition keywords (`fn`, `class`, `struct`, etc.) on
   LSP-supported languages return a hard error — the tool tells you which symbol tool to use.
```

- [ ] **Step 2: Verify anti-patterns table is still accurate** (~line 78)

The existing entries are already correct:
```
| `edit_file` with multi-line old_string on `.rs`/`.py`/`.ts` | `replace_symbol(...)` | Structural edits > fragile string matching |
| `edit_file` to delete a function | `remove_symbol(...)` | LSP knows the exact range |
| `edit_file` to add code after a function | `insert_code(...)` | Position-aware, no string matching |
```
These don't need changes — they already say "use symbol tools".

- [ ] **Step 3: Verify `edit_file` tool description** (~line 111)

The current description says:
```
- `edit_file(path, old_string, new_string)` — exact string replacement. Whitespace-sensitive.
  `replace_all=true` for all occurrences. `insert="prepend"|"append"` to add at file
  boundaries. For imports, literals, comments, config — NOT structural code changes.
```
This is already correct — no mention of `acknowledge_risk`. No change needed.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: update server instructions for edit_file hard block"
```

### Task 9: Final verification

- [ ] **Step 1: Run formatter**

Run: `cargo fmt`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS — no warnings.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Squash into a single commit if desired, or leave as-is**

The 4 commits tell a clean story. Leave as-is unless the user wants a squash.
