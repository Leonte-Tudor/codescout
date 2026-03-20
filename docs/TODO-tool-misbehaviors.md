# Tool Misbehaviors — Observed in the Wild

This is a living document. **Update it whenever you observe unexpected, wrong, or dangerous
behavior from codescout's own tools while working on the codebase.** Each entry should
capture: what you did, what you expected, what actually happened, and a reproduction hint.

---

## Prompt for future sessions

> Before starting any task on this codebase, re-read this file. While you work, watch for
> unexpected tool behavior: wrong edits, corrupt output, silent failures, misleading errors.
> When you find something, **add an entry here before continuing** — even a one-liner is
> enough to capture it while it's fresh. The goal is to build a corpus of real failure cases
> to drive test and UX improvements.

---

## Observed Bugs

### BUG-021 — `edit_file`: parallel calls cause partial state + MCP server "crash"

**Date:** 2026-03-03
**Severity:** High — leaves files in inconsistent partial state; server exit requires `/mcp` restart
**Status:** 🔍 ROOT CAUSE IDENTIFIED (2026-03-03) — two independent issues, one fixable

**What happened:**
Dispatched two `edit_file` calls in the same parallel response (targeting two different source
files: `src/embed/local.rs` and `src/config/project.rs`). The Claude Code permission system
handles each call independently: the first call was approved and returned `"ok"` (edit applied
to `local.rs`); the second call was rejected by the user and returned an error. This left the
two files in an inconsistent state — one edited, one not. Immediately after, the codescout
MCP server crashed and became unavailable, requiring a manual `/mcp` reconnect.

**Reproduction hint:**
1. Dispatch two `edit_file` tool calls in a single parallel response to different source files.
2. Approve the first permission prompt, reject (or let timeout) the second.
3. Observe: first file edited, second file unchanged — inconsistent partial state.
4. codescout MCP server crashes; subsequent tool calls fail until `/mcp` restart.

**Root cause (investigated 2026-03-03 — two separate issues):**

**Issue A — Partial state: inherent to independent parallel writes.**
When two `edit_file` calls target different files, they run as independent `tokio::spawn` tasks
inside rmcp's `serve_inner`. There is no transaction semantics across them. If one is denied
(permission dialog) while the other succeeds, the files are left in a partially-applied state.
This is NOT a bug in our code — it's the correct behavior for two independent operations. The
fix is operational: never dispatch parallel write tool calls.

**Issue B — "Crash" is actually Claude Code closing the stdio pipe (rmcp cancellation race).**
Static analysis of the full code path confirms there are NO panic paths in our production code
that could crash the server:
- All `lock().unwrap()` calls in the hot path (`open_files`, `OutputBuffer`) have trivial
  critical sections (HashSet ops only) — mutex cannot be poisoned by normal use.
- `call_tool_inner` routes ALL errors through `route_tool_error`; no unhandled panics.
- rmcp 0.1.5 spawns each request as `tokio::spawn` with the JoinHandle **dropped** — task
  panics are absorbed by the detached task and never propagate to the serve loop.
- The serve loop in `serve_inner` has no `unwrap()`/`expect()` in its event handler.

The "crash" is the server process exiting cleanly after the **stdio pipe closes**. This maps to
`service.waiting()` returning `QuitReason::Closed` → error propagates via `?` in `run()`.

**Why does Claude Code close the pipe?** Most likely a cancellation race in rmcp 0.1.5:
When Claude Code denies a parallel call, it may send a `notifications/cancelled` for the
in-flight request. rmcp cancels the `CancellationToken` but the spawned task has **no check**
for `context.ct.is_cancelled()` — it runs to completion and sends a response back through
`sink_proxy_tx`. The main loop then writes that response to stdout. Claude Code receives an
unexpected response for an already-cancelled request ID, which may cause it to close the
connection (a Claude Code MCP client bug, not ours).

**Fix:**
- **Operational** (immediate): never dispatch parallel write tool calls. Always finish one
  `edit_file` / `replace_symbol` / `insert_code` / `create_file` before starting the next.
- **rmcp limitation**: rmcp 0.1.5 does not suppress responses for cancelled requests.
  This cannot be fixed in our code without forking rmcp. Upgrading rmcp if a newer version
  respects cancellation tokens in the task-spawn path would help.
- **Defence-in-depth** (applied): `[profile.release] panic = "abort"` in Cargo.toml ensures
  any future panic kills the process cleanly rather than leaving a zombie server.

---

### BUG-026 — `read_file`: large ranged read on `@file_*` buffer ref silently wraps in `@tool_*`, breaking line navigation

**Date:** 2026-03-15
**Severity:** High — sub-range reads on large buffer refs return empty content, silently
**Status:** ✅ Fixed (2026-03-15)

**What happened:**
`read_file("@file_X", start_line=N, end_line=M)` where the extracted slice exceeds
`TOOL_OUTPUT_BUFFER_THRESHOLD` (≈10 KB) would return `{"output_id": "@tool_Y", "summary":
"511 lines...", "hint": "..."}`. Then reading `@tool_Y` with any `start_line > 4` returned
`{"content": "", "total_lines": 4}`.

**Root cause:**
Two-layer failure:
1. `read_file`'s buffer-ref path (`@file_*`/`@cmd_*`) returned the extracted content inline
   via `call()` — `json!({ "content": content, "total_lines": 511 })`.
2. `call_content()` (default `Tool` trait impl) serialized this and, because the JSON string
   exceeded the threshold, stored it as `@tool_*` via `store_tool()`.
3. Reading a `@tool_*` with `start_line`: the buffer content is the pretty-printed JSON
   `{"content": "line1\nline2\n...", "total_lines": 511}` — but `serde_json::to_string_pretty`
   keeps string values as single-line JSON with `\n` escapes, so the whole JSON is only 4
   lines. `total_lines = 4`, and any `start_line > 4` hits out-of-range → empty string.

The same root cause was fixed for the real-file explicit-range path in `BUG-025`, but the
buffer-ref path was missed.

**Reproduction:**
1. Read any file > ~10 KB: `read_file("path/to/large.md")` → `@file_X`
2. Range-read a large slice: `read_file("@file_X", start_line=1, end_line=300)` → `@tool_Y`
3. `read_file("@tool_Y", start_line=70, end_line=100)` → `{"content": "", "total_lines": 4}`

**Fix (src/tools/file.rs):**
In the buffer-ref line-range path, check `exceeds_inline_limit` after extracting lines. If
exceeded, call `output_buffer.store_file()` and return `{"file_id": "@file_Z", "total_lines":
N}` — a small JSON that `call_content()` won't re-buffer as `@tool_*`. Regression test:
`read_file_buffer_ref_large_range_buffers_as_file_ref`.

---

## Template for new entries

```
### BUG-XXX — `<tool name>`: <one-line description>

**Date:** YYYY-MM-DD
**Severity:** Low / Medium / High
**Status:** Open

**What happened:**
<what you did, what you expected, what happened instead>

**Reproduction hint:**
<minimal steps or context to reproduce>

**Root cause hypothesis:**
<your best guess at why it happened>

**Fix ideas:**
<options for fixing it in the tool or in its UX>

---
```

### BUG-027 — `replace_symbol` / `remove_symbol`: Kotlin LSP `range.start` lands mid-docstring, leaving unclosed `/**`

**Date:** 2026-03-18
**Severity:** High — silently corrupts Kotlin source files; causes cascading "Unclosed comment" + "Unresolved reference" compile errors
**Status:** ✅ Fixed (2026-03-18)

**What happened:**
Called `replace_symbol("createSolver", ...)` on a Kotlin file where `createSolver` had a
multi-line KDoc with preamble text before `@param` tags:

```kotlin
// line 106: /**
// line 107:  * Create a configured Stage1 solver for a specific tier.
// line 108:  *
// line 109:  * @param tier ...        ← kotlin-language-server reports range.start HERE
// line 110:  * @param lessonCount ...
// ...
// line 113:  */
// line 114: fun createSolver(
```

`replace_symbol` replaced from line 109 onward. Lines 106–108 (`/**`, description, blank `*`)
were left in the file. The new body also started with `/**`, producing two nested `/**`
openers with only one `*/` — an unclosed block comment. Kotlin compiler error:
`Syntax error: Unclosed comment at EOF`.

**Root cause:**
`kotlin-language-server` returns `DocumentSymbol.range.start` pointing to the first `@param`
tag line instead of the `/**` opener, when the KDoc has preamble text (description + blank
line) before its first `@` tag. Functions with short KDocs (no preamble, or only description,
no `@param`) are unaffected — their `range.start` correctly lands on `/**`.

codescout's `editing_start_line` (`src/tools/symbol.rs`) trusts `range_start_line`
(= `ds.range.start.line`) unconditionally. When it points mid-comment, `replace_symbol`
leaves the `/**` opener orphaned.

**Reproduction:**
1. Kotlin file with a function whose KDoc has preamble text before first `@param`:
   ```kotlin
   /**
    * Description paragraph.
    *
    * @param x ...
    */
   fun foo(x: Int) { ... }
   ```
2. Call `replace_symbol("foo", new_body_starting_with_/**/)`.
3. Observe: original `/**\n * Description paragraph.\n *\n` left in file; new `/**` appended.
4. Kotlin compiler reports "Unclosed comment".

**Fix (src/tools/symbol.rs — `editing_start_line`):**
When `range_start_line` is `Some(r)` and the line at `r` is inside a block comment
(starts with `*` after trimming), walk backward to find the `/**` opener:

```rust
fn editing_start_line(sym: &SymbolInfo, lines: &[&str]) -> usize {
    if let Some(r) = sym.range_start_line {
        let r = r as usize;
        // Kotlin LSP (and possibly others) may report range.start inside a /** */ block —
        // at the first @param line rather than the /** opener. Walk back to fix it.
        if r < lines.len() && lines[r].trim_start().starts_with('*') {
            for i in (0..r).rev() {
                if lines[i].trim_start().starts_with("/**") {
                    return i;
                }
            }
        }
        return r;
    }
    find_insert_before_line(lines, sym.start_line as usize)
}
```

Also add a Kotlin fixture test: function with multi-line KDoc + @params → assert
`body_start_line` == line of `/**`, not the `@param` line.

---

### BUG-028 — `create_file` / `edit_file`: does not notify LSP, leaving index stale

**Date:** 2026-03-18
**Severity:** Medium — after writing a file, `find_symbol` / `list_symbols` return stale results until LSP restarts
**Status:** ✅ Fixed (2026-03-18)

**What happened:**
After `create_file` rewrote a Kotlin fixture file with a new function added, subsequent
`list_symbols` and `find_symbol` calls did not return the new function. The Kotlin LSP was
still serving the pre-write symbol table. Only an `/mcp` reconnect (which kills and restarts
the LSP process) refreshed the index.

**Root cause:**
`create_file` (and `edit_file` for non-LSP-structural changes) writes directly to disk without
sending `textDocument/didChange` (or `didOpen` + `didChange`) to any running LSP client for
that file's language. The LSP only re-reads on the next `didOpen` — which only fires if the
file hasn't been opened before in the current session.

**Fix:**
After any write to a source file (`create_file`, `edit_file`), call
`ctx.lsp.notify_file_changed(&full_path).await` — the same `did_change` notification that
`replace_symbol` and `insert_code` already send. This ensures the LSP re-indexes the file
before the next `document_symbols` call.

Check: `create_file` and `edit_file` both already have access to `ctx.lsp` — just missing
the `notify_file_changed` call.

---

### BUG-029 — `insert_code`: `position: "after"` inserts inside function body instead of after it

**Date:** 2026-03-20
**Severity:** High — silently corrupts source files by splitting function bodies
**Status:** Open

**What happened:**
Called `insert_code("tests/write_produces_valid_framing", "src/lsp/transport.rs", code, "after")`
to add two new test functions after the last test in a `mod tests` block.

Expected: new functions inserted after the closing `}` of `write_produces_valid_framing`.

Actual: the new code was inserted **inside** `write_produces_valid_framing`'s body, splitting
the function in half. The original function's `let msg = json!(...)` ended up separated from
the rest of its body by the two new functions. Result:

```
    async fn write_produces_valid_framing() {
        let msg = json!({"test": true});

    #[tokio::test]                           // ← inserted HERE, inside the function
    async fn rejects_oversized_content_length() { ... }

    #[tokio::test]
    async fn accepts_normal_content_length() { ... }

        let mut buf = Vec::new();            // ← remainder of write_produces_valid_framing
        write_message(&mut buf, &msg).await.unwrap();
        ...
    }
```

Compiler warning: `cannot test inner items` (test functions defined inside another function).

**Reproduction:**
1. File with a `mod tests` block containing multiple `#[tokio::test]` async functions
2. Call `insert_code(name_path="tests/write_produces_valid_framing", path="src/lsp/transport.rs", code="<two test fns>", position="after")`
3. Observe: code lands inside the function body, not after it

**Root cause hypothesis:**
`insert_code` with `position: "after"` likely uses the LSP `DocumentSymbol.range.end` to find
the insertion point. For a function inside a `mod tests` block, the range may end at the last
line of the function body (before the closing `}`) rather than after it. Alternatively, the
insertion logic may be using `selection_range.end` (which points to the name) instead of
`range.end` (which should encompass the entire symbol including braces).

**Fix ideas:**
1. Verify that `insert_code` uses `range.end` (not `selection_range.end`) for the "after" position
2. After computing the insertion line, verify the line after it is outside the symbol's range
3. Add a test: `insert_code_after_places_code_after_closing_brace` with a multi-function mod block

---

### BUG-022 — Agent bypasses library tools, greps cargo registry directly

**Date:** 2026-03-16
**Severity:** Low — wasteful tokens, no data corruption
**Status:** 🔍 ROOT CAUSE IDENTIFIED

**What happened:**
When exploring rmcp's elicitation API, the agent used raw `run_command("grep ...")` on
`/home/marius/.cargo/registry/src/index.crates.io-*/rmcp-1.1.0/src/` instead of using
codescout's library tools (`register_library` + `find_symbol(scope="lib:rmcp")`).

**Expected:** Agent should register rmcp as a library and use structured symbol navigation.

**Actual:** 3 raw grep commands returning unstructured text, wasting context tokens.

**Root cause:** rmcp was not pre-registered as a library (`list_libraries` showed only
`anyhow`). The agent defaulted to the familiar grep pattern rather than first registering
the dependency and then using structured tools. The server instructions mention library
auto-discovery via `goto_definition`, but that requires navigating to an rmcp symbol first —
a chicken-and-egg problem when you don't yet know the API surface.

**Fix options:**
1. Auto-register top-N dependencies from `Cargo.lock` during `onboarding` or `activate_project`
2. Add a hint to `search_pattern` when it detects results in a cargo registry path:
   "Consider `register_library` + `find_symbol(scope=...)` for structured navigation"
3. Add `register_library` suggestion to server instructions for the "Know nothing" row
