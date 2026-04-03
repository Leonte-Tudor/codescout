# Embedding Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce default ONNX memory footprint from 158MB to 22MB per instance, remove silent Ollama fallback, and fix `openai:` API key handling.

**Architecture:** Three surgical changes to the embedding subsystem: swap the default model constant, delete the Ollama→local fallback block, and thread `api_key` through the `openai:` path. Plus doc/prompt surface updates.

**Tech Stack:** Rust, fastembed (ONNX), sqlite-vec

**Spec:** `docs/superpowers/specs/2026-04-03-embedding-simplification-design.md`

---

### Task 0: Enable local-embed feature by default

**Files:**
- Modify: `Cargo.toml:100` — default features list

- [ ] **Step 1: Add local-embed to default features**

In `Cargo.toml`, change line 100 from:

```toml
default = ["remote-embed", "dashboard", "http"]
```

to:

```toml
default = ["remote-embed", "local-embed", "dashboard", "http"]
```

This ensures `cargo install codescout` includes fastembed/ONNX support out of the box.
The ONNX model itself (22MB) only downloads on first use — not at compile or install time.

- [ ] **Step 2: Verify compile**

Run: `cargo build`
Expected: compiles with fastembed included.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat: include local-embed in default features for zero-config ONNX"
```

### Task 1: Change default model to AllMiniLML6V2Q

**Files:**
- Modify: `src/config/project.rs:287-289` — `default_embed_model()`
- Modify: `src/config/project.rs:361` — test `default_embed_model_is_nomic`
- Modify: `src/config/project.rs:367` — test `default_config_has_expected_embeddings`

- [ ] **Step 1: Update the default model function**

In `src/config/project.rs`, change `default_embed_model()`:

```rust
fn default_embed_model() -> String {
    "local:AllMiniLML6V2Q".into()
}
```

- [ ] **Step 2: Update tests**

In the same file, update the two tests that assert on `NomicEmbedTextV15Q`:

Test at line ~361: change expected value to `"local:AllMiniLML6V2Q"`.
Test at line ~367: change expected value to `"local:AllMiniLML6V2Q"`.

- [ ] **Step 3: Update doc comment on `EmbeddingsSection`**

In `src/config/project.rs:42-106`, update the struct doc comment:
- Remove the `custom:<model>@<base_url>` example (already removed in code).
- Change the recommended model from `NomicEmbedTextV15Q` to `AllMiniLML6V2Q`.
- Reorder the model list to put `AllMiniLML6V2Q` first as the default.

- [ ] **Step 4: Run tests**

Run: `cargo test -p codescout --lib config::project`
Expected: all tests pass with the new default.

- [ ] **Step 5: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(embed): change default model to AllMiniLML6V2Q (22MB)"
```

---

### Task 2: Remove Ollama fallback in create_embedder_with_config

**Files:**
- Modify: `src/embed/mod.rs:183-199` — `ollama:` branch fallback block
- Modify: `src/embed/mod.rs:138` — doc comment
- Modify: `src/embed/mod.rs:233,240` — error messages referencing NomicEmbedTextV15Q

- [ ] **Step 1: Write test for no-fallback behavior**

Add a test in `src/embed/mod.rs` tests module:

```rust
#[test]
fn ollama_prefix_without_server_returns_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // Use a port that's definitely not running Ollama
    std::env::set_var("OLLAMA_HOST", "http://127.0.0.1:19999");
    let result = rt.block_on(super::create_embedder_with_config(
        "ollama:nomic-embed-text",
        None,
        None,
    ));
    std::env::remove_var("OLLAMA_HOST");
    assert!(result.is_err(), "should error when Ollama is unreachable");
    let err = result.unwrap_err().to_string();
    assert!(
        !err.contains("Falling back"),
        "should NOT mention fallback: {err}"
    );
    assert!(
        err.contains("not reachable") || err.contains("Ollama"),
        "should mention Ollama is unreachable: {err}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout --lib embed::tests::ollama_prefix_without_server_returns_error`
Expected: FAIL — currently the code falls back to local instead of erroring.

- [ ] **Step 3: Remove the fallback block**

In `src/embed/mod.rs`, replace the `ollama:` branch (lines ~183-199) with:

```rust
    // 3. ollama: prefix (no fallback — errors if unreachable)
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("ollama:") {
        let host =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        if let Err(e) = remote::probe_ollama(&host).await {
            anyhow::bail!(
                "Ollama is not reachable at {host}: {e}\n\
                 Start Ollama or switch to a different embedding backend.\n\n\
                 Options:\n\
                 • url = \"http://your-server:port/v1\"    (any OpenAI-compatible endpoint)\n\
                 • model = \"local:AllMiniLML6V2Q\"        (bundled ONNX, 22MB, no server needed)"
            );
        }
        return Ok(Box::new(remote::RemoteEmbedder::ollama(model_id)?));
    }
```

- [ ] **Step 4: Update error messages and doc comment**

In the same file:
- Line ~138: change doc comment from `default to local:NomicEmbedTextV15Q` to `default to local:AllMiniLML6V2Q`.
- Line ~233: change `local:NomicEmbedTextV15Q (768d, quantized)` to `local:AllMiniLML6V2Q (384d, quantized, 22MB)`.
- Line ~240: change `local:NomicEmbedTextV15Q for bundled ONNX (768d, no server needed)` to `local:AllMiniLML6V2Q for bundled ONNX (384d, 22MB, no server needed)`.

- [ ] **Step 5: Update existing default test**

Rename `create_embedder_no_url_no_prefix_defaults_to_local_nomic` to `create_embedder_no_url_no_prefix_defaults_to_local_allminilm`. Update the test body to use `"AllMiniLML6V2Q"` instead of `"NomicEmbedTextV15Q"`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p codescout --lib embed::tests`
Expected: all tests pass, including the new no-fallback test.

- [ ] **Step 7: Commit**

```bash
git add src/embed/mod.rs
git commit -m "feat(embed): remove Ollama silent fallback to local ONNX"
```

---

### Task 3: Update local.rs error message

**Files:**
- Modify: `src/embed/local.rs:49-58` — `parse_model()` error message

- [ ] **Step 1: Update the error message**

In `src/embed/local.rs` `parse_model()`, reorder the model list and change the recommended label:

```rust
        other => anyhow::bail!(
            "Unknown local model '{other}'. Supported variants:\n\
             • local:AllMiniLML6V2Q               (384d, quantized, ~22MB, recommended default)\n\
             • local:NomicEmbedTextV15Q           (768d, quantized, ~158MB, higher quality)\n\
             • local:NomicEmbedTextV15            (768d, full precision, ~547MB)\n\
             • local:JinaEmbeddingsV2BaseCode     (768d, code-specific, ~300MB)\n\
             • local:AllMiniLML6V2                (384d, full precision)\n\
             • local:BGESmallENV15Q               (384d, deprecated — GPU-only, crashes on CPU)\n\
             • local:BGESmallENV15                (384d, full precision)"
        ),
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p codescout --lib embed::local`
Expected: all pass (error message content isn't asserted).

- [ ] **Step 3: Commit**

```bash
git add src/embed/local.rs
git commit -m "docs(embed): update local model list to recommend AllMiniLML6V2Q"
```

---

### Task 4: Thread api_key to openai: prefix path

**Files:**
- Modify: `src/embed/remote.rs:47-56` — `RemoteEmbedder::openai()`
- Modify: `src/embed/mod.rs` — `openai:` branch in `create_embedder_with_config()`

- [ ] **Step 1: Write test for config api_key**

Add a test in `src/embed/remote.rs` tests module (tests are in same module, so
`api_key` field is accessible):

```rust
#[test]
fn openai_uses_explicit_api_key_over_env() {
    let e = RemoteEmbedder::openai("text-embedding-3-small", Some("sk-from-config".into())).unwrap();
    assert_eq!(e.api_key.as_deref(), Some("sk-from-config"));
}
```

Also add the missing `url_with_ollama_prefix_strips_prefix` test in `src/embed/mod.rs` tests:

```rust
#[test]
fn url_with_ollama_prefix_strips_prefix() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(super::create_embedder_with_config(
        "ollama:nomic-embed-text",
        Some("http://localhost:11434/v1"),
        None,
    ));
    // Should succeed via URL path, not the ollama: branch
    assert!(result.is_ok(), "url+ollama: prefix should use URL path");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout --lib embed::remote::tests::openai_uses_explicit_api_key_over_env`
Expected: FAIL — `openai()` doesn't accept an api_key parameter yet.

- [ ] **Step 3: Update RemoteEmbedder::openai() signature**

In `src/embed/remote.rs`, change `openai()` to accept an optional api_key:

```rust
    pub fn openai(model: &str, api_key: Option<String>) -> Result<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| anyhow::anyhow!(
                "OpenAI API key not found. Set api_key in [embeddings] or OPENAI_API_KEY env var"
            ))?;
        Ok(Self {
            client: Self::http_client(),
            endpoint: "https://api.openai.com/v1/embeddings".into(),
            model: model.to_string(),
            api_key: Some(api_key),
        })
    }
```

- [ ] **Step 4: Update the caller in create_embedder_with_config**

In `src/embed/mod.rs`, the `openai:` branch (~line 206). Change:

```rust
    if let Some(model_id) = model.strip_prefix("openai:") {
        return Ok(Box::new(remote::RemoteEmbedder::openai(model_id, api_key)?));
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p codescout --lib embed`
Expected: all pass including the new test.

- [ ] **Step 6: Commit**

```bash
git add src/embed/remote.rs src/embed/mod.rs
git commit -m "feat(embed): openai prefix respects api_key from project.toml"
```

---

### Task 5: Update workflow.rs model options and onboarding version

**Files:**
- Modify: `src/tools/workflow.rs:15` — `ONBOARDING_VERSION`
- Modify: `src/tools/workflow.rs:57-108` — `model_options_for_hardware()`
- Modify: `src/tools/workflow.rs:5995-6084` — model_options tests

- [ ] **Step 1: Bump ONBOARDING_VERSION**

In `src/tools/workflow.rs:15`, change:

```rust
const ONBOARDING_VERSION: u32 = 3;  // was 2
```

- [ ] **Step 2: Update model_options_for_hardware()**

Change the first (default) option:

```rust
pub fn model_options_for_hardware(ctx: &HardwareContext) -> Vec<ModelOption> {
    let mut options = vec![ModelOption {
        id: "local:AllMiniLML6V2Q".into(),
        label: "AllMiniLML6V2Q".into(),
        dims: 384,
        context_tokens: 256,
        reason: "bundled ONNX, no server needed, lightweight default (22MB, quantized)".into(),
        available: true,
        recommended: true,
    }];
```

Also update the doc comment on line ~58 to say `AllMiniLML6V2Q`.

- [ ] **Step 3: Update tests**

Update all test assertions that check for `"local:NomicEmbedTextV15Q"` as the first option
(lines ~6004-6075) to expect `"local:AllMiniLML6V2Q"` instead. There are 4 assertions to change.

- [ ] **Step 4: Run tests**

Run: `cargo test -p codescout --lib tools::workflow::tests::model_options`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(embed): update onboarding model options to AllMiniLML6V2Q default"
```

---

### Task 6: Update prompts

**Files:**
- Modify: `src/prompts/onboarding_prompt.md:99` — "Ollama is the default" text

- [ ] **Step 1: Update onboarding prompt**

In `src/prompts/onboarding_prompt.md`, change line ~99 from:

```
Requires an embedding backend (Ollama is the default — see
```

to:

```
Requires an embedding backend (bundled ONNX is the default, Ollama/OpenAI optional — see
```

- [ ] **Step 2: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs(prompts): update onboarding to reflect local ONNX default"
```

---

### Task 7: Update manual pages

**Files:**
- Modify: `docs/manual/src/configuration/embeddings.md:16,123,164`
- Modify: `docs/manual/src/configuration/embedding-backends.md:90-110`
- Modify: `docs/manual/src/semantic-search-guide.md`

- [ ] **Step 1: Update embeddings.md**

In `docs/manual/src/configuration/embeddings.md`:
- Line 16: change `model = "local:NomicEmbedTextV15Q"` to `model = "local:AllMiniLML6V2Q"`.
- Line 123: change default value in config reference table to `"local:AllMiniLML6V2Q"`.
- Line 164: change `NomicEmbedTextV15Q` from "Recommended default" to just the description.
  Add "Recommended default" to the `AllMiniLML6V2Q` row instead.

- [ ] **Step 2: Update embedding-backends.md**

In `docs/manual/src/configuration/embedding-backends.md`:
- Remove or rewrite the "Automatic CPU Fallback" section (lines ~90-110).
  Replace with a note that `ollama:` prefix errors when Ollama is unreachable, with
  the same actionable hints from the error message (use `url` or `local:AllMiniLML6V2Q`).

- [ ] **Step 3: Update semantic-search-guide.md**

In `docs/manual/src/semantic-search-guide.md`:
- Update any references to the default model to say `AllMiniLML6V2Q`.
- Position it as lightweight default with upgrade path to Nomic/Jina/Ollama.

- [ ] **Step 4: Verify SUMMARY.md links**

Check `docs/manual/src/SUMMARY.md` — ensure all embeddings page links are intact after edits.
No content changes expected, just verify nothing broke.

- [ ] **Step 5: Commit**

```bash
git add docs/manual/src/configuration/embeddings.md \
        docs/manual/src/configuration/embedding-backends.md \
        docs/manual/src/semantic-search-guide.md
git commit -m "docs: update manual to reflect AllMiniLML6V2Q default and removed Ollama fallback"
```

---

### Task 8: Full test suite verification

- [ ] **Step 1: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all green.

- [ ] **Step 2: Build release and test via MCP**

Run: `cargo build --release`
Then restart the MCP server (`/mcp`) and verify:
- `index_status()` shows correct configured model
- `semantic_search("test query")` works
- Onboarding for a new project shows `AllMiniLML6V2Q` as default

- [ ] **Step 3: Squash into final commit if desired**

All tasks produce independent, well-tested commits. Squash or keep as-is per preference.
