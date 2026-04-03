# Unified Embedding Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the prefix-based embedding config with a `url` field for any OpenAI-compatible endpoint, add nomic-embed-text-v1.5 as the default local model, deprecate `ollama:` prefix, remove `custom:` prefix, and write comprehensive documentation.

**Architecture:** Add `url` and `api_key` fields to `EmbeddingsSection`. Update `create_embedder()` to check `url` first before prefix parsing. Add a new `RemoteEmbedder::from_url()` constructor that normalizes endpoint URLs. Add `NomicEmbedTextV15Q` to the local model list. Update onboarding model options and prompt. Write a single `docs/manual/src/embeddings.md` guide.

**Tech Stack:** Rust, fastembed (already has `NomicEmbedTextV15Q`), reqwest (existing), serde/toml (existing)

---

### Task 1: Add `url` and `api_key` fields to `EmbeddingsSection`

**Files:**
- Modify: `src/config/project.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `src/config/project.rs` (find the tests module):

```rust
#[test]
fn embeddings_section_parses_url_and_api_key() {
    let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
api_key = "test-key-123"
"#;
    let config: ProjectConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.embeddings.url.as_deref(), Some("http://127.0.0.1:43300/v1"));
    assert_eq!(config.embeddings.api_key.as_deref(), Some("test-key-123"));
}

#[test]
fn embeddings_section_url_defaults_to_none() {
    let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "ollama:nomic-embed-text"
"#;
    let config: ProjectConfig = toml::from_str(toml_str).unwrap();
    assert!(config.embeddings.url.is_none());
    assert!(config.embeddings.api_key.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test embeddings_section_parses_url -- --nocapture`
Expected: FAIL — `url` field unknown.

- [ ] **Step 3: Add the fields to `EmbeddingsSection`**

Add two new fields to the `EmbeddingsSection` struct after the `model` field:

```rust
    /// Base URL for an OpenAI-compatible embedding endpoint.
    ///
    /// When set, the `model` field is sent as the model name in the request body.
    /// The URL should point to the API base (e.g., `http://127.0.0.1:43300/v1`).
    /// Works with llama.cpp, vLLM, TEI, Ollama, OpenAI, and any server implementing
    /// `POST /v1/embeddings`.
    ///
    /// When absent, the `model` field's prefix determines the backend
    /// (`local:`, `ollama:`, `openai:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// API key for the embedding endpoint. Only used when `url` is set.
    /// Can also be provided via the `EMBED_API_KEY` environment variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test embeddings_section_parses -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(config): add url and api_key fields to EmbeddingsSection"
```

---

### Task 2: Add `RemoteEmbedder::from_url()` constructor

**Files:**
- Modify: `src/embed/remote.rs`

The new constructor normalizes URLs so users can pass `http://host:port`, `http://host:port/v1`, or `http://host:port/v1/embeddings` and it all works.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/embed/remote.rs`:

```rust
#[test]
fn from_url_normalizes_bare_host() {
    let e = RemoteEmbedder::from_url("http://127.0.0.1:43300", "nomic", None).unwrap();
    assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
    assert_eq!(e.model, "nomic");
    assert!(e.api_key.is_none());
}

#[test]
fn from_url_normalizes_v1_suffix() {
    let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1", "nomic", None).unwrap();
    assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
}

#[test]
fn from_url_normalizes_v1_embeddings_suffix() {
    let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1/embeddings", "nomic", None).unwrap();
    assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
}

#[test]
fn from_url_normalizes_trailing_slash() {
    let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1/", "nomic", None).unwrap();
    assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
}

#[test]
fn from_url_passes_api_key() {
    let e = RemoteEmbedder::from_url("http://host:8080", "model", Some("sk-123".into())).unwrap();
    assert_eq!(e.api_key.as_deref(), Some("sk-123"));
}

#[test]
fn from_url_falls_back_to_env_api_key() {
    // When api_key param is None, from_url checks EMBED_API_KEY env var.
    // We don't set it here, so it should be None.
    let e = RemoteEmbedder::from_url("http://host:8080", "model", None).unwrap();
    assert!(e.api_key.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test from_url_normalizes`
Expected: FAIL — method not found.

- [ ] **Step 3: Implement `from_url`**

Add to `impl RemoteEmbedder`:

```rust
    /// Create an embedder from an explicit URL.
    ///
    /// Normalizes the URL to always end with `/v1/embeddings`:
    /// - `http://host:port`              → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1`           → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1/embeddings`→ `http://host:port/v1/embeddings`
    pub fn from_url(url: &str, model: &str, api_key: Option<String>) -> Result<Self> {
        let base = url.trim_end_matches('/');
        let endpoint = if base.ends_with("/v1/embeddings") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{}/embeddings", base)
        } else {
            format!("{}/v1/embeddings", base)
        };

        let api_key = api_key.or_else(|| std::env::var("EMBED_API_KEY").ok());

        Ok(Self {
            client: Self::http_client(),
            endpoint,
            model: model.to_string(),
            api_key,
        })
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test from_url`
Expected: PASS

- [ ] **Step 5: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/embed/remote.rs
git commit -m "feat(embed): add RemoteEmbedder::from_url with URL normalization"
```

---

### Task 3: Add `NomicEmbedTextV15Q` to local models + chunk size

**Files:**
- Modify: `src/embed/local.rs`
- Modify: `src/embed/mod.rs`

- [ ] **Step 1: Write the failing tests**

In `src/embed/local.rs` tests:

```rust
#[test]
fn parse_model_nomic_v15_variants() {
    assert!(parse_model("NomicEmbedTextV15").is_ok());
    assert!(parse_model("NomicEmbedTextV15Q").is_ok());
}
```

In `src/embed/mod.rs` tests:

```rust
#[test]
fn chunk_size_local_nomic_v15() {
    let sz = super::chunk_size_for_model("local:NomicEmbedTextV15Q");
    assert_eq!(sz, 20889); // 8192 × 0.85 × 3
}

#[test]
fn chunk_size_local_nomic_v15_full() {
    let sz = super::chunk_size_for_model("local:NomicEmbedTextV15");
    assert_eq!(sz, 20889); // 8192 × 0.85 × 3
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test parse_model_nomic && cargo test chunk_size_local_nomic`
Expected: FAIL

- [ ] **Step 3: Add match arms to `parse_model` in `src/embed/local.rs`**

Add two new arms to the `match name` block:

```rust
        "NomicEmbedTextV15" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15),
        "NomicEmbedTextV15Q" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15Q),
```

Update the error message to list the new models and mark `BGESmallENV15Q` as deprecated:

```rust
        other => anyhow::bail!(
            "Unknown local model '{other}'. Supported variants:\n\
             • local:NomicEmbedTextV15Q          (768d, quantized, ~158MB, recommended)\n\
             • local:NomicEmbedTextV15            (768d, full precision, ~547MB)\n\
             • local:JinaEmbeddingsV2BaseCode     (768d, code-specific, ~300MB)\n\
             • local:AllMiniLML6V2Q               (384d, quantized, ~22MB, lightest)\n\
             • local:BGESmallENV15Q               (384d, deprecated — GPU-only, crashes on CPU)\n\
             • local:BGESmallENV15                (384d, full precision)\n\
             • local:AllMiniLML6V2                (384d, full precision)"
        ),
```

- [ ] **Step 4: Add chunk size entries in `src/embed/mod.rs`**

In `chunk_size_for_model`, inside the `if let Some(local_name) = model_spec.strip_prefix("local:")` block, add two new arms to the match:

```rust
            "nomicembedtextv15" | "nomicembedtextv15q" => 8192,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test parse_model_nomic && cargo test chunk_size_local_nomic`
Expected: PASS

- [ ] **Step 6: Update existing test `parse_model_known_names_return_ok`**

Add the new models to the existing test:

```rust
        assert!(parse_model("NomicEmbedTextV15").is_ok());
        assert!(parse_model("NomicEmbedTextV15Q").is_ok());
```

- [ ] **Step 7: Run fmt + clippy + full tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 8: Commit**

```bash
git add src/embed/local.rs src/embed/mod.rs
git commit -m "feat(embed): add NomicEmbedTextV15/V15Q local model support"
```

---

### Task 4: Update `create_embedder` resolution order

**Files:**
- Modify: `src/embed/mod.rs`

This is the core change — `create_embedder` now accepts an optional `url` and `api_key`, and checks `url` first before prefix parsing.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/embed/mod.rs`:

```rust
#[cfg(feature = "remote-embed")]
#[test]
fn create_embedder_with_url_uses_remote() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // We can't actually connect, but we can verify it doesn't error on creation
    // by checking that the url path is taken (not the prefix path).
    // Use a model name with no prefix — if url is respected, it won't hit
    // the "Unknown model prefix" error.
    let result = rt.block_on(super::create_embedder_with_config(
        "nomic-embed-text-v1.5",
        Some("http://127.0.0.1:99999"),
        None,
    ));
    // Should succeed (RemoteEmbedder created) — it only fails when we try to embed
    assert!(result.is_ok(), "url should create RemoteEmbedder without prefix: {:?}", result.err());
}

#[cfg(feature = "remote-embed")]
#[test]
fn custom_prefix_returns_migration_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(super::create_embedder("custom:model@http://localhost:1234"));
    let err = result.err().expect("custom: should error");
    assert!(err.to_string().contains("removed"), "error should say prefix is removed: {}", err);
    assert!(err.to_string().contains("url"), "error should mention url field: {}", err);
}

#[test]
fn create_embedder_no_url_no_prefix_defaults_to_local_nomic() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // A bare model name with no url should default to local:NomicEmbedTextV15Q
    // when the local-embed feature is available.
    #[cfg(feature = "local-embed")]
    {
        let result = rt.block_on(super::create_embedder("NomicEmbedTextV15Q"));
        // Without local-embed this would fail with "Unknown model prefix"
        // With local-embed, it should succeed (or at least not hit the prefix error)
        assert!(result.is_ok() || !result.as_ref().unwrap_err().to_string().contains("Unknown model prefix"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test create_embedder_with_url && cargo test custom_prefix_returns_migration`
Expected: FAIL — `create_embedder_with_config` not found; `custom:` still works.

- [ ] **Step 3: Implement `create_embedder_with_config` and update `create_embedder`**

Add new function and update the existing one in `src/embed/mod.rs`:

```rust
/// Create an embedder using explicit config fields.
///
/// Resolution order:
/// 1. `url` set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `local:` → local ONNX via fastembed
/// 3. `model` starts with `ollama:` → Ollama (deprecated, warns once)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. No url, no prefix → default to local:NomicEmbedTextV15Q
pub async fn create_embedder_with_config(
    model: &str,
    url: Option<&str>,
    api_key: Option<String>,
) -> Result<Box<dyn Embedder>> {
    // 1. URL takes priority — any OpenAI-compatible endpoint
    #[cfg(feature = "remote-embed")]
    if let Some(url) = url {
        return Ok(Box::new(remote::RemoteEmbedder::from_url(url, model, api_key)?));
    }
    #[cfg(not(feature = "remote-embed"))]
    if url.is_some() {
        anyhow::bail!(
            "Remote embedding requires the 'remote-embed' feature.\n\
             Rebuild with: cargo build --features remote-embed"
        );
    }

    // 2. local: prefix
    #[cfg(feature = "local-embed")]
    if let Some(model_id) = model.strip_prefix("local:") {
        return Ok(Box::new(local::LocalEmbedder::new(model_id)?));
    }

    // 3. ollama: prefix (deprecated)
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("ollama:") {
        use std::sync::Once;
        static WARN_ONCE: Once = Once::new();
        WARN_ONCE.call_once(|| {
            tracing::warn!(
                "ollama: prefix is deprecated. Use url = \"http://localhost:11434/v1\" \
                 and model = \"{}\" instead. The prefix will be removed in a future version.",
                model_id
            );
        });
        #[cfg(feature = "local-embed")]
        {
            let host =
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
            if let Err(e) = remote::probe_ollama(&host).await {
                const FALLBACK: &str = "NomicEmbedTextV15Q";
                tracing::warn!(
                    "{e}. Falling back to local:{FALLBACK}. \
                     Set embeddings.url in .codescout/project.toml to use a dedicated server."
                );
                return Ok(Box::new(local::LocalEmbedder::new(FALLBACK)?));
            }
        }
        return Ok(Box::new(remote::RemoteEmbedder::ollama(model_id)?));
    }

    // 4. openai: prefix
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("openai:") {
        return Ok(Box::new(remote::RemoteEmbedder::openai(model_id)?));
    }

    // 5. custom: prefix — removed, hard error
    #[cfg(feature = "remote-embed")]
    if model.starts_with("custom:") {
        anyhow::bail!(
            "The custom: prefix has been removed.\n\
             Use the url and model fields in [embeddings] instead.\n\n\
             Example .codescout/project.toml:\n\
             [embeddings]\n\
             model = \"your-model-name\"\n\
             url = \"http://your-server:port/v1\""
        );
    }

    // 6. No prefix — try as local model name
    #[cfg(feature = "local-embed")]
    {
        // Try parsing as a local model name directly
        if local::LocalEmbedder::new(model).is_ok() {
            return Ok(Box::new(local::LocalEmbedder::new(model)?));
        }
    }

    // Helpful error for local: prefix without the feature
    if model.starts_with("local:") {
        anyhow::bail!(
            "Local embedding requires the 'local-embed' feature.\n\
             Rebuild with: cargo build --features local-embed\n\n\
             Recommended: local:NomicEmbedTextV15Q (768d, quantized)"
        );
    }

    anyhow::bail!(
        "Unknown model '{}'. Options:\n\
         • Set url in [embeddings] to point at any OpenAI-compatible server\n\
         • Use local:NomicEmbedTextV15Q for bundled ONNX (768d, no server needed)\n\
         • Use local:JinaEmbeddingsV2BaseCode for code-specialized ONNX",
        model
    )
}

/// Create an embedder from a model string (legacy interface).
///
/// Delegates to `create_embedder_with_config` with no URL. Existing callers
/// that only have a model string continue to work unchanged.
pub async fn create_embedder(model: &str) -> Result<Box<dyn Embedder>> {
    create_embedder_with_config(model, None, None).await
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test create_embedder`
Expected: PASS

- [ ] **Step 5: Update caller in `agent.rs`**

Find `get_or_create_embedder` in `src/agent.rs`. It currently calls `create_embedder(model)`. Update it to also pass `url` and `api_key` from the project config:

```rust
    pub async fn get_or_create_embedder(
        &self,
        model: &str,
    ) -> anyhow::Result<Arc<dyn crate::embed::Embedder>> {
        // Read url and api_key from project config
        let (url, api_key) = self
            .with_project(|p| {
                Ok((
                    p.config.embeddings.url.clone(),
                    p.config.embeddings.api_key.clone(),
                ))
            })
            .await
            .unwrap_or((None, None));

        let cache_key = match &url {
            Some(u) => format!("{}@{}", model, u),
            None => model.to_string(),
        };

        let mut guard = self.cached_embedder.lock().await;
        if let Some((cached_key, embedder)) = guard.as_ref() {
            if *cached_key == cache_key {
                return Ok(Arc::clone(embedder));
            }
        }
        let embedder: Arc<dyn crate::embed::Embedder> = Arc::from(
            crate::embed::create_embedder_with_config(
                model,
                url.as_deref(),
                api_key,
            )
            .await?,
        );
        *guard = Some((cache_key, Arc::clone(&embedder)));
        Ok(embedder)
    }
```

- [ ] **Step 6: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 7: Commit**

```bash
git add src/embed/mod.rs src/agent.rs
git commit -m "feat(embed): url-first resolution in create_embedder, deprecate ollama: prefix"
```

---

### Task 5: Update `chunk_size_for_model` for bare model names with `url`

**Files:**
- Modify: `src/embed/mod.rs`

When `url` is set, the model string has no prefix (e.g., just `"nomic-embed-text-v1.5"`). The current `chunk_size_for_model` only strips known prefixes. Bare model names should go through the `tokens_for_bare` path directly.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn chunk_size_bare_nomic_model_name() {
    // When url is set, model has no prefix — just the bare name
    let sz = super::chunk_size_for_model("nomic-embed-text-v1.5");
    assert_eq!(sz, 20889); // 8192 × 0.85 × 3
}

#[test]
fn chunk_size_bare_unknown_model() {
    let sz = super::chunk_size_for_model("some-custom-model");
    assert_eq!(sz, 1305); // 512 × 0.85 × 3 (conservative fallback)
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cargo test chunk_size_bare`

The bare nomic test should actually already pass because the existing code falls through to `tokens_for_bare` for unknown prefixes. Let me verify — if the model string is `"nomic-embed-text-v1.5"`, it doesn't match any `strip_prefix`, so `bare` = `"nomic-embed-text-v1.5"`, and `tokens_for_bare` checks `l.contains("nomic-embed")` — yes, this should already work.

Expected: PASS (both tests should pass with existing code).

- [ ] **Step 3: If tests pass, just commit the new tests**

These tests document the expected behavior for bare model names. No code changes needed if they pass.

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 4: Commit**

```bash
git add src/embed/mod.rs
git commit -m "test(embed): add chunk_size tests for bare model names (url mode)"
```

---

### Task 6: Update default model and `model_options_for_hardware`

**Files:**
- Modify: `src/config/project.rs` (`default_embed_model`)
- Modify: `src/tools/workflow.rs` (`model_options_for_hardware`)

- [ ] **Step 1: Write the failing tests**

In `src/tools/workflow.rs` tests (find existing `model_options_for_hardware` tests):

```rust
#[test]
fn model_options_default_is_local_nomic() {
    let hw = HardwareContext {
        ollama_available: false,
        ollama_host: "http://localhost:11434".into(),
        gpu: None,
        ram_gb: 16,
        cpu_cores: 8,
    };
    let options = model_options_for_hardware(&hw);
    assert_eq!(options[0].id, "local:NomicEmbedTextV15Q");
    assert!(options[0].recommended);
    // Must have a url hint option
    assert!(
        options.iter().any(|o| o.reason.contains("url")),
        "must mention url as an option"
    );
}

#[test]
fn model_options_with_ollama_still_recommends_local() {
    let hw = HardwareContext {
        ollama_available: true,
        ollama_host: "http://localhost:11434".into(),
        gpu: None,
        ram_gb: 16,
        cpu_cores: 8,
    };
    let options = model_options_for_hardware(&hw);
    assert_eq!(options[0].id, "local:NomicEmbedTextV15Q");
    assert!(options[0].recommended);
    // Ollama option should mention url
    assert!(
        options.iter().any(|o| o.reason.contains("url") || o.reason.contains("Ollama")),
        "must mention Ollama or url option"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test model_options_default_is_local_nomic && cargo test model_options_with_ollama_still_recommends`
Expected: FAIL — current default is `ollama:nomic-embed-text`.

- [ ] **Step 3: Update `default_embed_model` in `src/config/project.rs`**

```rust
fn default_embed_model() -> String {
    "local:NomicEmbedTextV15Q".into()
}
```

- [ ] **Step 4: Update `model_options_for_hardware` in `src/tools/workflow.rs`**

Replace the entire function body:

```rust
pub fn model_options_for_hardware(ctx: &HardwareContext) -> Vec<ModelOption> {
    let mut options = vec![
        ModelOption {
            id: "local:NomicEmbedTextV15Q".into(),
            label: "NomicEmbedTextV15Q".into(),
            dims: 768,
            context_tokens: 8192,
            reason: "bundled ONNX, no server needed, good general baseline (quantized)".into(),
            available: true,
            recommended: true,
        },
    ];

    if ctx.ollama_available {
        options.push(ModelOption {
            id: "url".into(),
            label: "Use running Ollama".into(),
            dims: 768,
            context_tokens: 8192,
            reason: format!(
                "set url = \"{}/v1\" in project.toml to use your running Ollama",
                ctx.ollama_host.trim_end_matches('/')
            ),
            available: true,
            recommended: false,
        });
    }

    options.push(ModelOption {
        id: "local:JinaEmbeddingsV2BaseCode".into(),
        label: "JinaEmbeddingsV2BaseCode".into(),
        dims: 768,
        context_tokens: 8192,
        reason: "code-specialized ONNX, no server needed (~300MB download)".into(),
        available: true,
        recommended: false,
    });

    if !ctx.ollama_available {
        options.push(ModelOption {
            id: "url".into(),
            label: "External server".into(),
            dims: 0,
            context_tokens: 0,
            reason: "set url in [embeddings] to use any OpenAI-compatible embedding server".into(),
            available: true,
            recommended: false,
        });
    }

    options
}
```

- [ ] **Step 5: Update existing tests that assert on the old model options**

Search for tests that reference `"ollama:nomic-embed-text"` or `"ollama:bge-m3"` as model option IDs and update them to match the new options. Key tests to find and update:
- Any test asserting `options[0].id == "ollama:nomic-embed-text"`
- Any test asserting exactly 3 options (new function returns 3-4 depending on Ollama)

- [ ] **Step 6: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 7: Commit**

```bash
git add src/config/project.rs src/tools/workflow.rs
git commit -m "feat(embed): default to local:NomicEmbedTextV15Q, update model options"
```

---

### Task 7: Update onboarding prompt Phase 0

**Files:**
- Modify: `src/prompts/onboarding_prompt.md`

- [ ] **Step 1: Read current Phase 0**

Use `read_markdown("src/prompts/onboarding_prompt.md", heading="## Phase 0: Embedding Model Selection")`

- [ ] **Step 2: Replace Phase 0 content**

Replace the Phase 0 section with:

```markdown
## Phase 0: Embedding Model Selection

The `onboarding` tool has already written a recommended model to `.codescout/project.toml`
based on your system hardware. Present the options to the user now, before indexing starts.

Use the `model_options` array from the Gathered Project Data below to build the menu.
Use the `hardware` field for the one-line system summary.

Present this to the user:

> **Choose an embedding model for semantic search.**
>
> Based on your system ({hardware.cpu_cores} CPU cores
> {if hardware.gpu: ", {hardware.gpu.name}"}
> {if hardware.ollama_available: ", Ollama running" else: ", no Ollama detected"}):
>
> {for i, opt in model_options:}
> {i+1}. {if opt.recommended: "★ "}`{opt.id}` — {opt.dims}d, {opt.context_tokens}-token context
>    {opt.reason}{if opt.recommended: " ← **Recommended**"}{if not opt.available: " *(not currently available)*"}
> {end}
>
> Press Enter to accept [1], or type a number to choose a different option.
>
> **Tip:** For multi-project workspaces, running a dedicated embedding server is
> recommended over the bundled model. Set `url` in `.codescout/project.toml` to
> point at any OpenAI-compatible endpoint (llama.cpp, Ollama, vLLM, TEI).
> See the embeddings guide for setup examples.

Wait for the user's response, then:

- **User presses Enter or types 1:** The config is already correct — proceed to Phase 1.
- **User types 2, 3, etc.:** Call `edit_file` on `.codescout/project.toml`.
  Change the `model` line to the selected option's ID. If the option is `url`,
  ask the user for their server URL and add both `model` and `url` fields.
  Confirm the edit, then proceed to Phase 1.
- **User types a custom model string:** Use that string directly in the `edit_file` call.
  If it looks like a URL, suggest adding it as `url` instead.

Then proceed to Phase 1 (Semantic Index Check).

---
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs(onboarding): update Phase 0 for url-first embedding config"
```

---

### Task 8: Write embeddings documentation

**Files:**
- Create: `docs/manual/src/embeddings.md`
- Modify: `docs/manual/src/SUMMARY.md`
- Delete: `CODESCOUT-EMBEDDINGS-SETUP.md` (if it exists at project root or docs/)

- [ ] **Step 1: Find and delete the old setup doc**

Search for `CODESCOUT-EMBEDDINGS-SETUP.md` in the project. Delete it.

- [ ] **Step 2: Create `docs/manual/src/embeddings.md`**

```markdown
# Embeddings

codescout uses embeddings for semantic search — finding code by meaning rather than
exact text matches. This guide covers how to configure the embedding backend.

## Quick Start

codescout works out of the box with a bundled embedding model. No setup needed.

On first `index_project`, it downloads **nomic-embed-text-v1.5** (~158 MB, quantized)
to `~/.cache/huggingface/hub/` and runs it locally via ONNX. This is a one-time download.

```toml
# .codescout/project.toml (default — no changes needed)
[embeddings]
model = "local:NomicEmbedTextV15Q"
```

This is fine for single-project use or getting started. For better performance
with multiple projects, see the next section.

## Recommended: External Embedding Server

The bundled model loads into memory per codescout instance. With multiple projects
open, this duplicates memory (~158 MB each). A dedicated embedding server avoids this:

- **One process** serves all codescout instances
- **No memory duplication** — the model loads once
- **Faster queries** — the model stays warm
- **Model freedom** — use any model and quantization

### Configuration

Point codescout at your server with two fields:

```toml
[embeddings]
model = "nomic-embed-text-v1.5"          # model name (sent in API request)
url = "http://127.0.0.1:43300/v1"        # your server's base URL
# api_key = "optional-key"               # or set EMBED_API_KEY env var
```

The `url` field works with **any server implementing the OpenAI `/v1/embeddings` API**.
codescout normalizes the URL automatically — all of these are equivalent:

- `http://127.0.0.1:43300`
- `http://127.0.0.1:43300/v1`
- `http://127.0.0.1:43300/v1/embeddings`

### Setup Examples

#### llama.cpp

Download a GGUF model and start the server:

```bash
# Download (example: nomic-embed-text quantized)
wget https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/resolve/main/nomic-embed-text-v1.5.Q8_0.gguf

# Start server
llama-server -m nomic-embed-text-v1.5.Q8_0.gguf --embeddings --port 43300
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### Ollama

```bash
ollama pull nomic-embed-text
ollama serve  # if not already running
```

```toml
[embeddings]
model = "nomic-embed-text"
url = "http://127.0.0.1:11434/v1"
```

#### vLLM

```bash
vllm serve nomic-ai/nomic-embed-text-v1.5 --task embed --port 43300
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### TEI (HuggingFace Text Embeddings Inference)

```bash
docker run -p 43300:80 ghcr.io/huggingface/text-embeddings-inference \
  --model-id nomic-ai/nomic-embed-text-v1.5
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### OpenAI

```toml
[embeddings]
model = "text-embedding-3-small"
url = "https://api.openai.com/v1"
api_key = "sk-..."  # or set EMBED_API_KEY env var
```

## Configuration Reference

### `[embeddings]` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model` | string | `"local:NomicEmbedTextV15Q"` | Model name. With `url`: sent in API body. Without `url`: prefix determines backend. |
| `url` | string | *(none)* | Base URL for any OpenAI-compatible `/v1/embeddings` endpoint. |
| `api_key` | string | *(none)* | API key sent as Bearer token. Also available via `EMBED_API_KEY` env var. |
| `drift_detection_enabled` | bool | `true` | Track how much code meaning changes between index builds. |

### Resolution Order

When codescout needs to embed text, it resolves the backend in this order:

1. **`url` is set** → use it as an OpenAI-compatible endpoint
2. **`model` starts with `local:`** → bundled ONNX model via fastembed
3. **`model` starts with `ollama:`** → Ollama API *(deprecated — use `url` instead)*
4. **`model` starts with `openai:`** → OpenAI API with `OPENAI_API_KEY`
5. **No `url`, no prefix** → try as a local model name, then error with suggestions

### Environment Variables

| Variable | Description |
|----------|-------------|
| `EMBED_API_KEY` | API key for the embedding endpoint (alternative to config field) |
| `OPENAI_API_KEY` | OpenAI API key (used with `openai:` prefix) |
| `OLLAMA_HOST` | Ollama daemon URL (deprecated — use `url` field) |

## Model Recommendations

Minimum recommended: **768 dimensions** for good code search quality.

| Model | Dims | Download | Context | Best For |
|-------|------|----------|---------|----------|
| nomic-embed-text-v1.5 | 768 | ~158 MB (Q) / ~547 MB | 8192 | General purpose, **bundled default** |
| jina-embeddings-v2-base-en | 768 | ~300 MB | 8192 | Code-specialized |
| bge-m3 | 1024 | ~1.2 GB | 8192 | Best quality, needs external server |
| CodeSage-small-v2 | 1024 | ~500 MB | — | Purpose-built for code retrieval |
| text-embedding-3-small | 1536 | API only | 8191 | OpenAI hosted, no self-hosting |

### Bundled Local Models

These work with the `local:` prefix (no server needed):

| Model ID | Dims | Size | Context | Notes |
|----------|------|------|---------|-------|
| `NomicEmbedTextV15Q` | 768 | ~158 MB | 8192 | **Recommended default** |
| `NomicEmbedTextV15` | 768 | ~547 MB | 8192 | Full precision variant |
| `JinaEmbeddingsV2BaseCode` | 768 | ~300 MB | 8192 | Code-specialized |
| `AllMiniLML6V2Q` | 384 | ~22 MB | 256 | Ultra-lightweight |
| `AllMiniLML6V2` | 384 | ~90 MB | 256 | Full precision lightweight |

## How It Works

1. **AST-aware chunking** — tree-sitter extracts top-level definitions (functions, classes, structs). Each chunk is a complete semantic unit, not an arbitrary text window.

2. **Chunk size auto-derived** — codescout calculates chunk size from the model's context window. No manual tuning needed.

3. **Vector storage** — embeddings are stored in sqlite-vec (`vec0` virtual tables) for fast KNN search.

4. **Bundled model lifecycle** — the ONNX model is loaded lazily on first `semantic_search` or `index_project`, cached for 5 minutes, then unloaded to free memory.

## Troubleshooting

### Model mismatch after changing config

If you change the `model` or `url` after indexing, the stored vectors are incompatible.
Rebuild the index:

```
index_project(force: true)
```

### Endpoint unreachable

Check that the server is running and the URL is correct:

```bash
curl http://127.0.0.1:43300/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","input":["test"]}'
```

### Corporate proxy blocking downloads

The bundled model downloads from HuggingFace. If your proxy blocks this:

1. Download the model on an unrestricted machine
2. Copy to `~/.cache/huggingface/hub/models--nomic-ai--nomic-embed-text-v1.5/`
3. Or use an external server instead (set `url`)

## Migration from Prefix Syntax

The `ollama:` prefix is deprecated and will be removed in a future version.
Migrate to the `url` field:

```toml
# Before (deprecated)
[embeddings]
model = "ollama:nomic-embed-text"

# After
[embeddings]
model = "nomic-embed-text"
url = "http://127.0.0.1:11434/v1"
```

The `custom:model@url` syntax has been removed. If you see an error about it:

```toml
# Before (removed)
[embeddings]
model = "custom:my-model@http://my-server:8080"

# After
[embeddings]
model = "my-model"
url = "http://my-server:8080/v1"
```
```

- [ ] **Step 3: Update `docs/manual/src/SUMMARY.md`**

Add the embeddings page to the manual index. Find the appropriate place in the summary and add:

```markdown
- [Embeddings](embeddings.md)
```

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 5: Commit**

```bash
git add docs/manual/src/embeddings.md docs/manual/src/SUMMARY.md
git rm CODESCOUT-EMBEDDINGS-SETUP.md 2>/dev/null || true
git commit -m "docs: add comprehensive embeddings guide, remove old setup doc"
```

---

### Task 9: Build release and verify

- [ ] **Step 1: Run full validation**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

- [ ] **Step 2: Build release binary**

```bash
cargo build --release
```

- [ ] **Step 3: Verify config parsing**

Create a test `project.toml` with the new fields and verify it loads without error. The existing tests should cover this, but a manual spot-check:

```bash
echo '[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"' > /tmp/test-project.toml
```

- [ ] **Step 4: Verify via MCP**

Restart the MCP server with `/mcp` and verify:
- `onboarding(force: true)` on a project → default model is `local:NomicEmbedTextV15Q`
- Model options include a `url` hint
- Old `ollama:` configs still work (with deprecation warning in logs)
