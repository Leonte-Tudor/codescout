# Dependency Upgrades to Latest Major Versions

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade 8 Rust dependencies to their latest major versions, one crate at a time, with tests passing after each.

**Architecture:** Sequential per-crate bumps in ascending order of risk. Each crate is its own commit. Compile errors drive the fixup — `cargo check` surfaces all breaking changes immediately. Tests confirm correctness after each migration.

**Tech Stack:** Rust, Cargo, rusqlite (SQLite FFI), rmcp (MCP SDK), tree-sitter (AST), reqwest (HTTP), fastembed (ONNX local embeddings).

---

## Upgrade Order & Rationale

| # | Crate | Old | New | Risk | Why that order |
|---|-------|-----|-----|------|----------------|
| 1 | `thiserror` | 1.x | 2.x | ☆ trivial | Not actually used in source; just a Cargo.toml entry |
| 2 | `toml` | 0.8 | 1.0 | ☆ low | API surface (`from_str`/`to_string_pretty`/`Value`) is stable; many call sites but mechanical |
| 3 | `git2` | 0.19 | 0.20 | ☆ low | Contained in one file (`src/git/mod.rs`) |
| 4 | `reqwest` | 0.12 | 0.13 | ☆ low | One file, behind `remote-embed` feature flag |
| 5 | `fastembed` | 4.x | 5.x | ★ medium | One file, behind `local-embed` feature; model variant names may change |
| 6 | `tree-sitter` | 0.25 | 0.26 | ★ medium | Two files + 6 grammar crates; grammar ABI may block this (see Task 6) |
| 7 | `rusqlite` | 0.31 | 0.39 | ★★ high | Many files, unsafe FFI for sqlite-vec extension loading |
| 8 | `rmcp` + `schemars` | 0.1 + 0.8 | 1.2 + 1.x | ★★ high | Server core; SSE transport obsoleted; model type API likely changed |

---

## Before You Start

- [ ] Read `docs/TODO-tool-misbehaviors.md` for any known tool issues.
- [ ] Confirm you're on the `experiments` branch: `git branch --show-current`
- [ ] Confirm tests pass at baseline: `cargo test 2>&1 | tail -5`

---

## Task 1: thiserror 1 → 2

**Files:**
- Modify: `Cargo.toml` (one line)

`thiserror` is listed in `Cargo.toml` but not imported anywhere in `src/`. It's a zero-risk bump.

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`, change:
  ```toml
  thiserror = "1"
  ```
  to:
  ```toml
  thiserror = "2"
  ```

- [ ] **Step 2: Build and test**

  ```bash
  cargo check 2>&1 | grep -E "^error" | head -20
  cargo test 2>&1 | tail -10
  ```
  Expected: no errors.

- [ ] **Step 3: Commit**

  ```bash
  git add Cargo.toml Cargo.lock
  git commit -m "chore: upgrade thiserror 1 → 2"
  ```

---

## Task 2: toml 0.8 → 1.0

**Files:**
- Modify: `Cargo.toml`
- Possibly: `src/config/project.rs`, `src/config/workspace.rs`, `src/workspace.rs`,
  `src/memory/anchors.rs`, `src/agent.rs`, `src/tools/file_summary.rs`,
  `src/tools/workflow.rs`, `src/tools/config.rs`

The API surface used is `toml::from_str`, `toml::to_string_pretty`, `toml::Value`, `toml::Table`.
In toml 1.0, `to_string_pretty` was renamed/restructured and `Value::try_from` semantics may differ.
The authoritative source for breaking changes: <https://docs.rs/toml/1.0.0/toml/>

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`:
  ```toml
  toml = "1"
  ```

- [ ] **Step 2: Compile and collect all errors**

  ```bash
  cargo check 2>&1 | grep "^error" | head -40
  ```

- [ ] **Step 3: Fix each compile error**

  Common toml 1.0 migration patterns:
  - `toml::to_string_pretty(&x)` → still exists; check if return type changed (`Result<String, _>`)
  - `toml::from_str::<T>(&s)` → still exists; error type may differ
  - `toml::Value::Table(t)` → still valid; `toml::Table` is `IndexMap<String, Value>`

  Fix errors file by file. Run `cargo check` after each file to confirm progress.

- [ ] **Step 4: Run tests**

  ```bash
  cargo test 2>&1 | tail -15
  ```
  Expected: all tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/
  git commit -m "chore: upgrade toml 0.8 → 1.0"
  ```

---

## Task 3: git2 0.19 → 0.20

**Files:**
- Modify: `Cargo.toml`
- Possibly: `src/git/mod.rs`, `src/embed/index.rs` (test helpers only)

`git2` 0.20 updates `libgit2` to 1.9. The Rust API surface we use is minimal:
`Repository::discover`, `Repository::init` (tests), `DiffOptions`, `DiffFindOptions`,
`Delta` enum variants, `Oid::from_str`, `Repository` revwalk, basic commit/tree access.

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`:
  ```toml
  git2 = "0.20"
  ```

- [ ] **Step 2: Compile and fix errors**

  ```bash
  cargo check 2>&1 | grep "^error" | head -40
  ```

  Check the git2 changelog for 0.20 at <https://github.com/rust-lang/git2-rs/blob/master/CHANGELOG.md>.
  Most usage is basic and unlikely to break.

- [ ] **Step 3: Run tests**

  ```bash
  cargo test 2>&1 | tail -15
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/
  git commit -m "chore: upgrade git2 0.19 → 0.20"
  ```

---

## Task 4: reqwest 0.12 → 0.13

**Files:**
- Modify: `Cargo.toml`
- Possibly: `src/embed/remote.rs`

`remote.rs` uses: `reqwest::Client`, `Client::builder()`, `.json(&body)`, `.await`, response `.json::<T>()`.
reqwest 0.13 is a TLS/runtime refactor — the async API surface is mostly unchanged.

Key breaking change in 0.13: `ClientBuilder::build()` may require explicit TLS backend selection.
Check: <https://github.com/seanmonstar/reqwest/releases/tag/v0.13.0>

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`:
  ```toml
  reqwest = { version = "0.13", features = ["json"], optional = true }
  ```

- [ ] **Step 2: Compile (remote-embed feature)**

  ```bash
  cargo check --features remote-embed 2>&1 | grep "^error" | head -20
  ```

- [ ] **Step 3: Fix any errors in `src/embed/remote.rs`**

- [ ] **Step 4: Run tests (all features)**

  ```bash
  cargo test --features remote-embed 2>&1 | tail -15
  ```

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/embed/remote.rs
  git commit -m "chore: upgrade reqwest 0.12 → 0.13"
  ```

---

## Task 5: fastembed 4 → 5

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/embed/local.rs`

`local.rs` uses: `fastembed::TextEmbedding`, `fastembed::InitOptions::new(model)`,
`fastembed::EmbeddingModel` variants (`JinaEmbeddingsV2BaseCode`, `BGESmallENV15Q`,
`AllMiniLML6V2Q`, `BGESmallENV15`, `AllMiniLML6V2`).

fastembed 5.x may rename `EmbeddingModel` variants or change `InitOptions` API.
Check: <https://github.com/Anush008/fastembed-rs/blob/main/CHANGELOG.md>

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`:
  ```toml
  fastembed = { version = "5", optional = true }
  ```

- [ ] **Step 2: Compile under local-embed feature**

  ```bash
  cargo check --features local-embed 2>&1 | grep "^error" | head -20
  ```

- [ ] **Step 3: Fix errors in `src/embed/local.rs`**

  Common changes to watch for:
  - `InitOptions::new(model)` → may be builder pattern now
  - `EmbeddingModel` variant names — check fastembed docs for renamed/removed variants
  - `TextEmbedding::embed()` return type / batch API

- [ ] **Step 4: Run tests**

  ```bash
  cargo test --features local-embed 2>&1 | tail -15
  ```

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/embed/local.rs
  git commit -m "chore: upgrade fastembed 4 → 5"
  ```

---

## Task 6: tree-sitter 0.25 → 0.26 (+ grammar crates)

**Files:**
- Modify: `Cargo.toml`
- Possibly: `src/ast/parser.rs`, `src/embed/ast_chunker.rs`

**⚠ Grammar crate availability warning:** As of 2026-03-15, the grammar crates on
crates.io (`tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, etc.) are
still at their 0.24/0.25-compatible versions. **If they have not been updated for
tree-sitter 0.26, this task may not be possible yet.** See the abort condition below.

The API we use: `tree_sitter::{Node, Parser, Language}`, `Parser::new()`,
`parser.set_language(&lang)`, `tree_sitter_rust::LANGUAGE.into()`.

tree-sitter 0.26 breaking change: `Language` construction changed from
`tree_sitter_rust::LANGUAGE.into()` to `tree_sitter_rust::LANGUAGE.into()` — likely
still the same, but verify. Check <https://github.com/tree-sitter/tree-sitter/releases>.

- [ ] **Step 1: Check grammar crate availability**

  ```bash
  cargo search tree-sitter-rust 2>&1 | head -3
  cargo search tree-sitter-python 2>&1 | head -3
  ```

  If the grammar crates are still at 0.24/0.25 with no 0.26-compatible release:
  **→ Skip this task.** Add a note to `docs/TODO-tool-misbehaviors.md` and move on.

- [ ] **Step 2: Bump tree-sitter and grammar crates**

  In `Cargo.toml`, bump `tree-sitter` and all grammar crates to their latest versions.
  Check <https://crates.io/crates/tree-sitter-rust> for the latest grammar versions.

  ```toml
  tree-sitter = "0.26"
  tree-sitter-rust = "0.26.x"      # use whatever is published
  tree-sitter-python = "0.26.x"
  tree-sitter-go = "0.26.x"
  tree-sitter-typescript = "0.26.x"
  tree-sitter-java = "0.26.x"
  tree-sitter-kotlin-ng = "..."    # check crates.io
  ```

- [ ] **Step 3: Compile**

  ```bash
  cargo check 2>&1 | grep "^error" | head -30
  ```

  Key files to fix: `src/ast/parser.rs`, `src/embed/ast_chunker.rs`.

- [ ] **Step 4: Run tests**

  ```bash
  cargo test 2>&1 | tail -15
  ```

  The tree-sitter tests in `src/tools/symbol.rs` exercise the fallback path — confirm
  these still pass.

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/
  git commit -m "chore: upgrade tree-sitter 0.25 → 0.26 and grammar crates"
  ```

---

## Task 7: rusqlite 0.31 → 0.39

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/embed/drift.rs`, `src/embed/index.rs`, `src/usage/db.rs`,
  `src/tools/semantic.rs`, `src/tools/memory.rs`

This is a large jump (8 minor versions). Key API surface in use:
- `rusqlite::{params, Connection}` — params macro, `Connection::open`, `execute`, `query_row`, `prepare`
- `OptionalExtension` trait — `.optional()` on query results
- `rusqlite::Row<'_>` in row mapper closures
- `rusqlite::Error::QueryReturnedNoRows` — explicit error variant match
- **Unsafe FFI**: `rusqlite::ffi::sqlite3_auto_extension` + `sqlite3` + `sqlite3_api_routines` in `src/embed/index.rs` for loading the `sqlite-vec` extension

Check the rusqlite changelog: <https://github.com/rusqlite/rusqlite/blob/master/CHANGELOG.md>

Key breaking changes to watch for in 0.32–0.39:
- `params!` macro API is stable; likely unchanged
- `Connection::open` signature stable
- Row mapping closure: `|row: &Row<'_>|` — check if `Row` lifetime changed
- `OptionalExtension` may have moved crate path
- `ffi` module: `sqlite3_auto_extension` function pointer signature may have tightened (unsafe transmute in `index.rs` is the riskiest spot)
- `rusqlite::Error` variant set may have changed

- [ ] **Step 1: Bump the version**

  In `Cargo.toml`:
  ```toml
  rusqlite = { version = "0.39", features = ["bundled"] }
  ```

- [ ] **Step 2: Compile and capture all errors**

  ```bash
  cargo check 2>&1 | grep "^error" | head -50
  ```

  Work through errors file by file starting with the simplest (`src/usage/db.rs`,
  `src/tools/memory.rs`, `src/tools/semantic.rs`) before tackling `src/embed/index.rs`.

- [ ] **Step 3: Fix the FFI extension-loading code (`src/embed/index.rs`)**

  The critical unsafe block loads the `sqlite-vec` extension:
  ```rust
  rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
      ...,
      *mut rusqlite::ffi::sqlite3,
      *const rusqlite::ffi::sqlite3_api_routines,
  >(...)));
  ```

  If the `ffi` types changed, update the transmute target types to match the new
  `rusqlite::ffi` definitions. Verify the `sqlite-vec` crate version is still compatible —
  check: <https://crates.io/crates/sqlite-vec>.

- [ ] **Step 4: Run full test suite**

  ```bash
  cargo test 2>&1 | tail -20
  ```

  Pay attention to any database-related test failures — they surface subtle API drift
  that compiles but behaves wrong (e.g., changed SQL error mapping).

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/
  git commit -m "chore: upgrade rusqlite 0.31 → 0.39"
  ```

---

## Task 8: rmcp 0.1 → 1.2 (+ schemars 0.8 → 1.x)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/server.rs` — `ServerHandler`, transport, `McpError`, `CallToolResult`, `ListToolsResult`, `RequestContext<RoleServer>`, SSE server
- Modify: `src/tools/mod.rs` — `Content` type, `CallToolResult` construction
- Modify: `src/tools/progress.rs` — progress reporting API
- Modify: `src/tools/workflow.rs` — `rmcp::model::Content::text()`
- Modify: `src/usage/mod.rs` — `rmcp::model::Content`

This is the biggest migration. rmcp went from an experimental pre-1.0 API to a stable 1.x.

**Known breaking change:** `transport-sse-server` feature is **obsolete** in rmcp 1.2 — SSE transport was restructured. The SSE server used at `src/server.rs:440` (`rmcp::transport::sse_server::SseServer`) must be migrated to the new transport API.

Reference:
- rmcp 1.x docs: <https://docs.rs/rmcp/1.2.0/rmcp/>
- rmcp changelog: <https://github.com/modelcontextprotocol/rust-sdk/blob/main/CHANGELOG.md>

- [ ] **Step 1: Update Cargo.toml features**

  ```toml
  # Remove transport-sse-server from rmcp features — it's obsolete in 1.x
  rmcp = { version = "1", features = ["server", "macros", "transport-io"] }
  schemars = "1"
  ```

  Check rmcp 1.x docs to confirm the correct feature names for stdio and SSE transports.

- [ ] **Step 2: Compile and capture all errors**

  ```bash
  cargo check 2>&1 | grep "^error" | head -60
  ```

  There will likely be many errors. Group them by file.

- [ ] **Step 3: Fix `src/tools/mod.rs` and `src/usage/mod.rs`**

  These use `rmcp::model::Content`. In rmcp 1.x, `Content` type may have moved or
  its constructor (`Content::text()`) may have changed signature.

  Fix imports and construction sites. Run:
  ```bash
  cargo check 2>&1 | grep "src/tools/mod\|src/usage/mod" | head -20
  ```

- [ ] **Step 4: Fix `src/tools/progress.rs`**

  The progress reporter uses rmcp notification types. Check the new rmcp 1.x progress
  notification API and update accordingly.

- [ ] **Step 5: Fix `src/server.rs` — model types**

  Fix `CallToolRequestParam`, `CallToolResult`, `McpError`, `ServerInfo`,
  `ListToolsResult`, `RequestContext<RoleServer>`, `PaginatedRequestParam`, `ServerHandler`.

  These are the core MCP protocol types — in 1.x they are stabilised but names/paths
  may have shifted. Update all imports at the top of `server.rs`.

- [ ] **Step 6: Fix `src/server.rs` — SSE transport**

  The current SSE code (around line 440):
  ```rust
  let mut sse_server = rmcp::transport::sse_server::SseServer::serve(addr)
  ```

  In rmcp 1.x, SSE transport was restructured. Check the rmcp 1.x examples:
  <https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples>

  Update the SSE serve block to the new API. If SSE is fully removed, update the
  `"sse"` transport arm in `run()` to return an unsupported error or use the new
  equivalent.

- [ ] **Step 7: Run full test suite**

  ```bash
  cargo test 2>&1 | tail -20
  ```

  The `server_registers_all_tools`, `all_tools_have_valid_schemas`, and
  `recoverable_error_routes_to_success_not_is_error` tests are the key validators.

- [ ] **Step 8: Manual MCP smoke test**

  ```bash
  cargo build --release
  # then restart MCP server: /mcp
  # call list_tools and one simple tool (e.g. read_file on a markdown file)
  ```

- [ ] **Step 9: Commit**

  ```bash
  git add Cargo.toml Cargo.lock src/
  git commit -m "chore: upgrade rmcp 0.1 → 1.2, schemars 0.8 → 1.x"
  ```

---

## Final Verification

- [ ] **Full test suite on clean build**

  ```bash
  cargo clean && cargo test 2>&1 | tail -20
  ```

- [ ] **Lint**

  ```bash
  cargo fmt && cargo clippy -- -D warnings 2>&1 | grep "^error" | head -20
  ```

- [ ] **Release build**

  ```bash
  cargo build --release 2>&1 | tail -5
  ```

- [ ] **Confirm dependency count**

  ```bash
  cargo outdated --depth 1 2>&1
  ```

  Expected: no major-version outdated entries remaining.
