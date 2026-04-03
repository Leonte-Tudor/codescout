# Unified Embedding Configuration Design

**Date:** 2026-03-31
**Status:** Approved
**Goal:** Replace the prefix-based embedding config with a simple `url` field that works with any OpenAI-compatible endpoint, improve the default local model, and write thorough documentation.

---

## 1. Config Format

The `[embeddings]` section in `.codescout/project.toml` gains two new optional fields:

```toml
[embeddings]
model = "nomic-embed-text-v1.5"          # model name (sent in API body)
url = "http://127.0.0.1:43300/v1"        # any OpenAI-compatible /v1/embeddings endpoint
# api_key = ""                            # optional, or EMBED_API_KEY env var
```

### Resolution Order

When `create_embedder()` is called:

1. **`url` is set** → `RemoteEmbedder` targeting that URL. `model` is the model name sent in the request body. `api_key` or `EMBED_API_KEY` env var used if present.
2. **`model` starts with `local:`** → bundled ONNX via fastembed. No server needed.
3. **`model` starts with `ollama:`** → Ollama native API. **Deprecated** — log a one-time warning per session: `"ollama: prefix is deprecated. Use url = \"http://localhost:11434/v1\" and model = \"<name>\" instead."` Still works.
4. **`model` starts with `openai:`** → OpenAI API with `OPENAI_API_KEY`. Same as `url = "https://api.openai.com/v1"`.
5. **`model` starts with `custom:`** → **Hard error**: `"custom: prefix removed. Use url and model fields instead. Example: url = \"http://host:port/v1\", model = \"name\""`
6. **No `url`, no prefix** → default to `local:NomicEmbedTextV15Q`.

### Backward Compatibility

- Existing `ollama:model` configs keep working with a deprecation warning.
- Existing `openai:model` configs keep working (no change).
- Existing `custom:model@url` configs produce a clear error with migration instructions.
- Configs without `url` field deserialize normally (field is `Option<String>`).

---

## 2. New Default Local Model

### Replace weak fallbacks with nomic-embed-text-v1.5

**Removed:**
- `BGESmallENV15Q` — deprecated (GPU-only FP16, crashes on CPU despite "Q" suffix).

**New default:**
- `local:NomicEmbedTextV15` — nomic-embed-text-v1.5 ONNX (full precision, ~547 MB)
  - 768 dimensions, 8192 token context
  - One-time download via HuggingFace hub to `~/.cache/huggingface/hub/`
  - Apache 2.0 license
  - Matryoshka Representation Learning (MRL) — dimensions can be truncated if needed
- `local:NomicEmbedTextV15Q` — quantized variant (dynamic quantization, smaller download)
  - Same 768d/8192ctx, reduced size via ONNX dynamic quantization
  - Preferred for most users (smaller, fast on CPU)

**Kept:**
- `local:JinaEmbeddingsV2BaseCode` — code-specialized alternative (768d, ~300 MB)
- `local:AllMiniLML6V2Q` — ultra-lightweight (384d, ~22 MB)
- `local:AllMiniLML6V2` — full precision lightweight
- `local:BGESmallENV15` — full precision (CPU-safe, unlike the Q variant)

### fastembed Integration

fastembed 4.0.0 already includes `NomicEmbedTextV15` and `NomicEmbedTextV15Q` variants. Just add match arms in `parse_model()` and update the model list in error messages. fastembed handles tokenization + pooling + ONNX download.

---

## 3. Memory Management for Local Embedder

### The Multi-Instance Problem

codescout runs as a separate MCP server per project. With 10-20 instances, loading a 158 MB ONNX model in each is wasteful (158 MB × 20 = 3.2 GB).

### Strategy: Lazy Load + Idle Timeout

- **Not loaded at startup.** Zero memory cost for idle instances.
- **Loaded on first use** (`semantic_search` or `index_project`).
- **Kept warm for 5 minutes** after last use (fast repeated queries).
- **Dropped after 5 minutes idle** (memory reclaimed).

This means:
- A single `semantic_search` incurs a ~1-2s cold load, then subsequent queries are ~50ms.
- After 5 minutes of no embedding activity, the model is unloaded.
- At most, only actively-used instances hold the model in memory.

### Documentation Guidance

The bundled ONNX model is a **zero-config fallback**. For users with multiple projects or frequent semantic search usage, the documentation will recommend running a dedicated embedding server (llama.cpp, Ollama, vLLM, TEI) and pointing at it with `url`. Benefits:
- Single process serves all codescout instances (no memory duplication)
- Faster queries (model always warm)
- Freedom to choose any model and quantization

---

## 4. Onboarding Updates

### `model_options_for_hardware()` Changes

**When Ollama is available:**

| # | Option | Details |
|---|--------|---------|
| 1 | `local:NomicEmbedTextV15Q` (recommended) | 768d, 158 MB, no daemon needed |
| 2 | `url` + Ollama | "Set `url = \"http://localhost:11434/v1\"` to use your running Ollama" |
| 3 | `local:JinaEmbeddingsV2BaseCode` | 768d, code-specialized, 300 MB |

**When Ollama is NOT available:**

| # | Option | Details |
|---|--------|---------|
| 1 | `local:NomicEmbedTextV15Q` (recommended) | 768d, 158 MB, no daemon needed |
| 2 | `local:JinaEmbeddingsV2BaseCode` | 768d, code-specialized, 300 MB |
| 3 | "Set `url` to use an existing embedding server" | Points to documentation |

Note: `ollama:` prefix is no longer recommended in onboarding. The default config writes `local:NomicEmbedTextV15`. Users who want Ollama are directed to use `url`.

### Onboarding Prompt Update

Update Phase 0 (Embedding Model Selection) in `src/prompts/onboarding_prompt.md` to:
- Present the `url` option for users with existing infrastructure
- Explain that bundled ONNX is a fallback, external server is preferred for multi-project setups
- Remove references to `ollama:` as the primary recommendation

---

## 5. Documentation

### New: `docs/manual/src/embeddings.md`

Single comprehensive guide with these sections:

#### 5.1 Quick Start
- Works out of the box with bundled `nomic-embed-text-v1.5` ONNX model
- No setup needed — first `index_project` downloads ~158 MB model once
- Good for single-project use or getting started

#### 5.2 Recommended: External Embedding Server
- Why: shared process for all instances, no memory duplication, faster queries, model freedom
- Copy-paste setup examples:

**llama.cpp (GGUF, any GPU):**
```bash
llama-server -m nomic-embed-text-v1.5.Q8_0.gguf --embeddings --port 43300
```
```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

**Ollama:**
```bash
ollama pull nomic-embed-text
```
```toml
[embeddings]
model = "nomic-embed-text"
url = "http://127.0.0.1:11434/v1"
```

**vLLM:**
```bash
vllm serve nomic-ai/nomic-embed-text-v1.5 --task embed --port 43300
```
```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

**TEI (HuggingFace Text Embeddings Inference):**
```bash
docker run -p 43300:80 ghcr.io/huggingface/text-embeddings-inference \
  --model-id nomic-ai/nomic-embed-text-v1.5
```
```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

**OpenAI:**
```toml
[embeddings]
model = "text-embedding-3-small"
url = "https://api.openai.com/v1"
api_key = "sk-..."   # or set EMBED_API_KEY env var
```

#### 5.3 Configuration Reference
- Full `[embeddings]` field reference: `model`, `url`, `api_key`, `drift_detection_enabled`
- Resolution order (url > prefix > default)
- Environment variables: `EMBED_API_KEY`, `OLLAMA_HOST` (deprecated)

#### 5.4 Model Recommendations

| Model | Dims | Size | Context | Best For |
|-------|------|------|---------|----------|
| nomic-embed-text-v1.5 | 768 | 158 MB (bnb4) | 8192 | General purpose, bundled default |
| jina-embeddings-v2-base-en | 768 | ~300 MB | 8192 | Code-specialized |
| bge-m3 | 1024 | ~1.2 GB | 8192 | Best quality, needs server |
| CodeSage-small-v2 | 1024 | ~500 MB | — | Purpose-built for code |
| text-embedding-3-small | 1536 | API | 8191 | OpenAI, no self-hosting |

Minimum recommended: 768 dimensions for good code search quality.

#### 5.5 How It Works Internally
- AST-aware chunking (tree-sitter extracts top-level definitions)
- Chunk size auto-derived from model context window
- Vectors stored in sqlite-vec (vec0 virtual tables) for KNN search
- Bundled model: lazy-loaded, cached 5 minutes, then unloaded
- External server: just HTTP calls, no memory overhead

#### 5.6 Troubleshooting
- Model mismatch after changing config → `index_project(force: true)`
- Endpoint unreachable → check URL, firewall, server running
- Corporate proxy blocking HuggingFace downloads → manual cache setup
- `custom:` prefix error → migration instructions

#### 5.7 Migration from Prefix Syntax
- `ollama:model` → `url = "http://localhost:11434/v1"`, `model = "model"` (deprecated, still works with warning)
- `custom:model@url` → `url = "url"`, `model = "model"` (removed, hard error)
- `openai:model` → works as-is, or use `url = "https://api.openai.com/v1"`

### Deleted: `CODESCOUT-EMBEDDINGS-SETUP.md`
This file is outdated (WSL2/Zscaler workarounds). Relevant troubleshooting content folded into the new guide.

### Updated: `docs/manual/src/SUMMARY.md`
Add the new embeddings page to the manual index.

---

## 6. Code Changes

### `src/config/project.rs`
- Add `url: Option<String>` to `EmbeddingsSection`
- Add `api_key: Option<String>` to `EmbeddingsSection`
- Both default to `None`, backward-compatible deserialization

### `src/embed/mod.rs`
- Update `create_embedder()` with new resolution order (Section 1)
- `custom:` prefix → `anyhow::bail!` with migration message
- `ollama:` prefix → `tracing::warn!` deprecation (once per session via `std::sync::Once` or similar)

### `src/embed/local.rs`
- Add `NomicEmbedTextV15` and `NomicEmbedTextV15Q` match arms to `parse_model()`
- Both map to fastembed's existing `EmbeddingModel::NomicEmbedTextV15` / `NomicEmbedTextV15Q`
- Add to `chunk_size_for_model()` known models table: `context_tokens = 8192`, `dimensions = 768`
- Update error message to list new models
- Deprecate `BGESmallENV15Q` in the error message (add "(deprecated, GPU-only)" note)

### `src/embed/local.rs` or new `src/embed/cache.rs`
- Lazy-load + 5-minute idle timeout for the ONNX model
- `Arc<Mutex<Option<(TextEmbedding, Instant)>>>` — model + last-used timestamp
- Background task or check-on-use to evict after timeout
- Shared across calls within the same codescout process

### `src/tools/workflow.rs`
- Update `model_options_for_hardware()` per Section 4
- Default config writes `local:NomicEmbedTextV15` instead of `ollama:nomic-embed-text`
- Third option in all scenarios mentions `url`

### `src/prompts/onboarding_prompt.md`
- Update Phase 0 to present `url` as the recommended path for power users
- Bundled ONNX as zero-config default
- Remove `ollama:` as primary recommendation

### `src/prompts/server_instructions.md`
- If it references embedding configuration, update to mention `url` field

---

## 7. Tests

### Config parsing
- `url` field present → parsed correctly
- `url` field absent → `None`, backward compatible
- `api_key` field parsed
- Existing configs without new fields deserialize without error

### Embedder resolution
- `url` set → `RemoteEmbedder` with that URL
- `local:NomicEmbedTextV15` → `LocalEmbedder` with correct model
- `ollama:model` → works but logs deprecation warning
- `custom:model@url` → error with migration message
- No url, no prefix → defaults to `local:NomicEmbedTextV15`

### Onboarding
- `model_options_for_hardware` with Ollama → shows local default + url hint
- `model_options_for_hardware` without Ollama → shows local default + url hint
- Default config writes `local:NomicEmbedTextV15`

### Local embedder cache
- First call loads model
- Subsequent calls reuse loaded model
- After timeout, model is dropped
