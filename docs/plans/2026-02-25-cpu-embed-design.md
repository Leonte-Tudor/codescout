# CPU-Friendly Local Embedding Design

**Date:** 2026-02-25
**Status:** Approved

## Problem

The current default embedding backend (`ollama:mxbai-embed-large`) requires a running
Ollama daemon — a separate install and process. Users without GPU, or on WSL2, have no
zero-dependency path to semantic search. The `local:` prefix is already reserved in
`create_embedder()` but bails with a "rebuild with candle" error pointing at a
commented-out, never-implemented feature.

The insight driving this: AST-aware chunking produces function-level, semantically
dense chunks with doc+signature prefixes. The signal per chunk is high, so a lightweight
model produces good results without needing a heavy GPU model.

## Decisions

- **Default stays** `ollama:mxbai-embed-large` — no behavior change for existing users
- **`local:` prefix** implemented via `fastembed-rs` (ONNX Runtime, CPU-optimized,
  auto-downloads models from HuggingFace on first use)
- **Model names** use fastembed's `EmbeddingModel` enum variant names directly
  (e.g. `local:JinaEmbeddingsV2BaseCode`) — transparent, discoverable, future-proof
- **Model mismatch** → clear error message; user deletes the DB and re-runs

## Architecture

### New: `src/embed/local.rs` (gated by `local-embed` feature)

```rust
pub struct LocalEmbedder {
    model: fastembed::TextEmbedding,
    dims: usize,
}

impl LocalEmbedder {
    pub fn new(model_name: &str) -> Result<Self>;  // parses fastembed enum name
}

impl Embedder for LocalEmbedder { ... }
```

Model name parsing uses a `match` over supported variant names. Unknown names
produce an error listing supported options.

Model cache: fastembed's default `~/.cache/huggingface/hub/` — shared with Python
tooling, works on WSL2, no config needed.

### Modified: `src/embed/mod.rs`

`create_embedder()` gains a `local:` branch (behind `#[cfg(feature = "local-embed")]`):

```rust
#[cfg(feature = "local-embed")]
if let Some(model_id) = model.strip_prefix("local:") {
    return Ok(Box::new(local::LocalEmbedder::new(model_id)?));
}
```

The existing bail for the missing-feature case is updated with helpful model names.

### Modified: `src/embed/index.rs` — Model Tracking

New `meta` table in SQLite (added to `open_db()`):

```sql
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Two helpers: `get_meta(conn, key)` and `set_meta(conn, key, value)`.

`build_index()` logic at startup:

```
read meta["embed_model"]
  → None    : first run → proceed → write model string at end
  → matches : proceed normally
  → differs : bail! "Index was built with '{stored}'. Configured model is
               '{current}'. Delete .code-explorer/embeddings.db and re-run."
```

This guard applies to **all backends** (ollama, openai, custom, local).

`index_stats()` returns the stored model name; surfaced by the `index_status` tool.

### Modified: `Cargo.toml`

Replace commented-out candle block:

```toml
fastembed = { version = "4", optional = true }

[features]
default = ["remote-embed"]
remote-embed = ["dep:reqwest"]
local-embed  = ["dep:fastembed"]
```

### Modified: `src/config/project.rs`

Updated doc comment on `EmbeddingsSection.model`:

```
/// "local:<EmbeddingModel variant>"  → fastembed-rs, CPU/WSL2, no daemon needed
///
/// Recommended local models (build with --features local-embed):
///   "local:JinaEmbeddingsV2BaseCode"   → 768d, code-specific, ~300MB
///   "local:BGESmallENV15Q"             → 384d, quantized, ~20MB, fast
///   "local:AllMiniLML6V2Q"             → 384d, quantized, ~22MB, lightest
///   "local:SnowflakeArcticEmbedXSQ"    → 384d, quantized, tiny but strong
```

## Recommended Models

| Model string | Dims | Download | Notes |
|---|---|---|---|
| `local:JinaEmbeddingsV2BaseCode` | 768 | ~300MB | Code-specific training, best quality |
| `local:BGESmallENV15Q` | 384 | ~20MB | Quantized, fast CPU, good general quality |
| `local:AllMiniLML6V2Q` | 384 | ~22MB | Lightest, used by cocoindex-code as default |
| `local:SnowflakeArcticEmbedXSQ` | 384 | ~22M params | Ultra-tiny, strong MTEB performance |

**Recommended default** when advising CPU/WSL2 users: `local:JinaEmbeddingsV2BaseCode`
(code-specific training justifies the larger download for a code search tool).

## Out of Scope

- `--reindex` CLI flag (future convenience; for now, delete the DB manually)
- Configurable model cache location
- Progress reporting during model download (fastembed handles this internally)
