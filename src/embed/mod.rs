//! Embedding engine: semantic code search via local or remote embeddings.
//!
//! Inspired by cocoindex-code (../cocoindex-code/) but implemented natively
//! in Rust with sqlite-vec for zero-dependency vector storage.
//!
//! Architecture:
//!   chunker → Embedder trait → sqlite-vec index
//!
//! Two Embedder backends:
//!   - LocalEmbedder  (fastembed/ONNX, feature "local-embed") — fully offline, CPU/WSL2-friendly
//!   - RemoteEmbedder (reqwest, feature "remote-embed")   — OpenAI-compatible API

pub mod ast_chunker;
pub mod chunker;
pub mod drift;
pub mod index;
pub mod schema;

#[cfg(feature = "remote-embed")]
pub mod remote;

#[cfg(feature = "local-embed")]
pub mod local;

use anyhow::Result;

/// Embedding vector — dimensions depend on the configured model
/// (e.g. 768 for jina-embeddings-v2-base-code, 384 for bge-small).
pub type Embedding = Vec<f32>;

/// Trait implemented by all embedding backends.
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    /// Return the dimensionality of the produced vectors.
    fn dimensions(&self) -> usize;

    /// Embed a batch of texts, returning one vector per text.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>;
}

/// Returns the chunk size in characters appropriate for the given model spec.
///
/// Derived from each model's documented maximum sequence length using a
/// conservative formula: `max_tokens × 0.85 × 3 chars/token`.
///
/// - The 0.85 factor leaves 15 % headroom for tokenisation variance and
///   control tokens (BOS/EOS).
/// - Code tokenises at roughly 3–4 chars/token; 3 is the conservative lower
///   bound, ensuring chunks stay within the context window even for files with
///   many short identifiers and operators.
///
/// Unknown or custom models fall back to 512 tokens (the most common context
/// window among small embedding models). This is intentionally conservative —
/// chunks will be smaller than necessary but will never be truncated.
///
/// This value is not user-configurable. It is derived from the model spec
/// so that users cannot accidentally misconfigure it.
pub fn chunk_size_for_model(model_spec: &str) -> usize {
    // 85 % of context × 3 chars/token.
    fn from_tokens(n: usize) -> usize {
        (n as f64 * 0.85 * 3.0) as usize
    }

    // Map well-known model name substrings to their published max sequence
    // lengths. Matching is done on the bare model name (prefix stripped) so
    // that "ollama:nomic-embed-text" and "openai:nomic-embed-text" both match.
    fn tokens_for_bare(name: &str) -> usize {
        let l = name.to_lowercase();
        // 8 192-token models
        if l.contains("nomic-embed") || l.contains("jina") || l.contains("bge-m3") {
            return 8192;
        }
        // OpenAI text-embedding-3-* and text-embedding-ada-002
        if l.starts_with("text-embedding-") {
            return 8191;
        }
        // mxbai-embed-large (MixedBread)
        if l.contains("mxbai") {
            return 512;
        }
        // BGE Small variants
        if l.contains("bge-small") || l.starts_with("bge_small") {
            return 512;
        }
        // all-MiniLM-L6-v2
        if l.contains("all-minilm") || l.contains("minilm-l6") {
            return 256;
        }
        // Unknown — conservative fallback
        512
    }

    // Local fastembed models use their documented sequence lengths.
    // These are listed here rather than in local.rs to avoid a feature-gate
    // dependency (local.rs is #[cfg(feature = "local-embed")]).
    if let Some(local_name) = model_spec.strip_prefix("local:") {
        let max_tokens = match local_name.to_lowercase().as_str() {
            "nomicembedtextv15" | "nomicembedtextv15q" => 8192,
            "jinaembeddingsv2basecode" => 8192,
            "bgesmallenv15q" | "bgesmallenv15" => 512,
            "allminilml6v2q" | "allminilml6v2" => 256,
            _ => 512,
        };
        return from_tokens(max_tokens);
    }

    // Strip backend prefix to get the bare model name.
    let bare = model_spec
        .strip_prefix("ollama:")
        .or_else(|| model_spec.strip_prefix("openai:"))
        .or_else(|| {
            // "custom:model-name@base_url" — extract only the model-name part
            model_spec
                .strip_prefix("custom:")
                .map(|rest| rest.split('@').next().unwrap_or(rest))
        })
        .unwrap_or(model_spec);

    from_tokens(tokens_for_bare(bare))
}

/// Convenience extension for embedding a single text.
pub async fn embed_one(embedder: &dyn Embedder, text: &str) -> Result<Embedding> {
    let mut batch = embedder.embed(&[text]).await?;
    batch
        .pop()
        .ok_or_else(|| anyhow::anyhow!("Embedder returned empty batch"))
}

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
        // Strip known routing prefixes so "ollama:nomic-embed-text" + url
        // sends "nomic-embed-text" as the model name in the HTTP request.
        let bare_model = model
            .strip_prefix("ollama:")
            .or_else(|| model.strip_prefix("openai:"))
            .or_else(|| model.strip_prefix("local:"))
            .unwrap_or(model);
        return Ok(Box::new(remote::RemoteEmbedder::from_url(
            url, bare_model, api_key,
        )?));
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

#[cfg(test)]
mod tests {

    #[test]
    fn unknown_prefix_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("bogus:model"));
        let err = result.err().expect("expected an error");
        assert!(
            err.to_string().contains("Unknown model"),
            "unexpected error: {}",
            err
        );
    }

    #[cfg(not(feature = "local-embed"))]
    #[test]
    fn local_prefix_returns_helpful_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("local:anything"));
        let err = result.err().expect("expected an error");
        assert!(err.to_string().contains("local-embed"));
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn custom_prefix_missing_at_sign_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("custom:no-at-sign"));
        let err = result.err().expect("expected an error");
        // custom: prefix is now removed — error should be the migration hint
        assert!(
            err.to_string().contains("removed"),
            "unexpected error: {}",
            err
        );
        assert!(
            err.to_string().contains("url"),
            "error should mention url field: {}",
            err
        );
    }

    // ---------- chunk_size_for_model ----------

    #[test]
    fn chunk_size_mxbai_embed_large() {
        // Default model: 512-token context. Formula: 512 × 0.85 × 3 = 1305.
        let sz = super::chunk_size_for_model("ollama:mxbai-embed-large");
        assert_eq!(sz, 1305);
    }

    #[test]
    fn chunk_size_nomic_embed_text() {
        // 8 192-token context. Formula: 8192 × 0.85 × 3 = 20 889.
        let sz = super::chunk_size_for_model("ollama:nomic-embed-text");
        assert_eq!(sz, 20889);
    }

    #[test]
    fn chunk_size_bge_m3() {
        // bge-m3 has 8192-token context. Formula: 8192 × 0.85 × 3 = 20889.
        let sz = super::chunk_size_for_model("ollama:bge-m3");
        assert_eq!(sz, 20889);
    }

    #[test]
    fn chunk_size_openai_text_embedding_3_small() {
        let sz = super::chunk_size_for_model("openai:text-embedding-3-small");
        assert_eq!(sz, 20887); // 8191 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_jina() {
        let sz = super::chunk_size_for_model("local:JinaEmbeddingsV2BaseCode");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_bge_small() {
        let sz = super::chunk_size_for_model("local:BGESmallENV15Q");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_all_minilm() {
        let sz = super::chunk_size_for_model("local:AllMiniLML6V2Q");
        assert_eq!(sz, 652); // 256 × 0.85 × 3
    }

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

    #[test]
    fn chunk_size_custom_model() {
        // custom: prefix with @url — model name extracted before @
        let sz = super::chunk_size_for_model("custom:mxbai-embed-large@http://localhost:1234");
        assert_eq!(sz, 1305);
    }

    #[test]
    fn chunk_size_unknown_model_falls_back_to_512_tokens() {
        let sz = super::chunk_size_for_model("ollama:some-unknown-model");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3
    }

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
        assert!(
            result.is_ok(),
            "url should create RemoteEmbedder without prefix: {:?}",
            result.err()
        );
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn custom_prefix_returns_migration_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("custom:model@http://localhost:1234"));
        let err = result.err().expect("custom: should error");
        assert!(
            err.to_string().contains("removed"),
            "error should say prefix is removed: {}",
            err
        );
        assert!(
            err.to_string().contains("url"),
            "error should mention url field: {}",
            err
        );
    }

    #[test]
    fn create_embedder_no_url_no_prefix_defaults_to_local_allminilm() {
        // A bare model name with no url should be accepted as a local model
        // when the local-embed feature is available.
        #[cfg(feature = "local-embed")]
        {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(super::create_embedder("AllMiniLML6V2Q"));
            assert!(result.is_ok(), "AllMiniLML6V2Q should load as local model");
        }
    }

    #[test]
    fn chunk_size_bare_nomic_model_name() {
        // When url is set, model has no prefix — just the bare name.
        // This test documents that bare model names work correctly.
        let sz = super::chunk_size_for_model("nomic-embed-text-v1.5");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_bare_unknown_model() {
        // When url is set, custom model names with no prefix fall back to
        // the conservative 512-token default.
        let sz = super::chunk_size_for_model("some-custom-model");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3 (conservative fallback)
    }
}
