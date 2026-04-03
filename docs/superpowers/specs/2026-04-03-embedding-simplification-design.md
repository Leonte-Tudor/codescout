# Embedding Simplification Design

> Supersedes: `2026-03-31-unified-embedding-config-design.md`
> Status: Approved
> Date: 2026-04-03

## Problem

1. **Memory bloat**: Default local model (`NomicEmbedTextV15Q`, 158MB) loads per instance.
   With 10 codescout instances, that's 1.58GB — too much for users with 8-16GB RAM.
2. **Silent fallback**: `ollama:` prefix silently loads a 158MB local model when Ollama
   is unreachable. Users don't know they're running a degraded (and expensive) fallback.
3. **API key inconsistency**: `openai:` prefix reads `OPENAI_API_KEY` env var but ignores
   the `api_key` field in `project.toml`.

## Design

### Configuration Paths (priority order)

| Config | Behavior | API key resolution |
|---|---|---|
| `url` set | `RemoteEmbedder` via URL. No local model loaded. | `api_key` in config → `EMBED_API_KEY` env |
| `openai:model` | `RemoteEmbedder` via OpenAI API | `api_key` in config → `OPENAI_API_KEY` env |
| `ollama:model` | `RemoteEmbedder` via `$OLLAMA_HOST`. **No fallback to local.** | None needed |
| `local:model` | `LocalEmbedder` ONNX via fastembed | N/A |
| No config | `local:AllMiniLML6V2Q` (22MB, 384d) | N/A |

### What Changes

#### 1. Default model: `AllMiniLML6V2Q` (22MB) replaces `NomicEmbedTextV15Q` (158MB)

In `src/config/project.rs`, `default_embed_model()` returns `"local:AllMiniLML6V2Q"`.

**Tradeoff**: 384d embeddings produce weaker semantic search than 768d. Acceptable because:
- Most users never configure embeddings — they get a working default at minimal cost.
- Users who care about quality will configure a URL or explicit `local:NomicEmbedTextV15Q`.
- 22MB × 10 instances = 220MB vs 1.58GB — 7x improvement.

#### 2. Remove Ollama fallback to local

In `src/embed/mod.rs` `create_embedder_with_config()`, the `ollama:` prefix branch currently:
1. Probes Ollama
2. If unreachable, falls back to `local:NomicEmbedTextV15Q`

**Change**: Remove step 2. If Ollama is unreachable, return an error:

```
Ollama is not reachable at {host}.
Start Ollama or switch to a different embedding backend.

Options:
  • url = "http://your-server:port/v1"    (any OpenAI-compatible endpoint)
  • model = "local:AllMiniLML6V2Q"        (bundled ONNX, 22MB, no server needed)

See: https://github.com/mareurs/codescout/wiki/Embeddings
```

The `probe_ollama` function stays (useful for onboarding diagnostics) but no longer
triggers a fallback code path.

#### 3. `openai:` prefix respects `api_key` from config

In `src/embed/remote.rs` `RemoteEmbedder::openai()`, change API key resolution from:

```
OPENAI_API_KEY env var only
```

to:

```
api_key parameter (from project.toml) → OPENAI_API_KEY env var
```

This requires threading the `api_key` from `EmbeddingsSection` through to the `openai:`
branch in `create_embedder_with_config()`. The function signature already accepts
`api_key: Option<String>` — it just isn't passed to the `openai:` path today.

#### 4. Documentation: update existing manual pages

Update existing embedding documentation pages (not create new ones):

- `docs/manual/src/configuration/embeddings.md` — update default model, configuration reference.
- `docs/manual/src/configuration/embedding-backends.md` — remove "Automatic CPU Fallback"
  section, update Ollama docs to reflect error-on-unreachable.
- `docs/manual/src/semantic-search-guide.md` — update to reflect `AllMiniLML6V2Q` as default.

Content to cover across these pages:
- **Quick start**: works out of the box with bundled ONNX, no setup needed.
- **Upgrading quality**: model comparison table showing size/dims/quality tradeoffs.
- **Using Ollama**: recipe with `model = "ollama:nomic-embed-text"`, explain `OLLAMA_HOST`.
- **Using OpenAI**: recipe with `model = "openai:text-embedding-3-small"`, explain API key options.
- **Custom server**: recipe with `url` + `model` for llama.cpp, vLLM, TEI, etc.
- **Troubleshooting**: model mismatch, Ollama unreachable, re-indexing.

### Edge case: `url` set with a prefix

When `url` is set, `create_embedder_with_config` takes the URL path (priority 1) and
strips any `ollama:`, `openai:`, or `local:` prefix from the model name before sending
the bare name in the HTTP request body. This is the existing behavior and does not change.

**Note:** `url` + `local:AllMiniLML6V2Q` will strip the prefix and send
`AllMiniLML6V2Q` as the model name to the remote server, which is almost certainly
wrong. This is a pre-existing issue — documenting it in the manual's troubleshooting
section is sufficient for now.

### What Does NOT Change

- `ollama:` and `openai:` prefixes stay as convenience shortcuts.
- `local:` prefix stays.
- `url` + `api_key` config fields stay.
- `EMBED_API_KEY` / `OPENAI_API_KEY` env vars stay as fallbacks.
- `custom:` prefix stays as hard error (already removed previously).
- `probe_ollama` stays for diagnostics.
- No lazy load / idle timeout — out of scope for this change.

## Code Changes

### `src/config/project.rs`

- `default_embed_model()` → return `"local:AllMiniLML6V2Q"` instead of `"local:NomicEmbedTextV15Q"`.
- Update `EmbeddingsSection` doc comments to reflect new default, remove stale `custom:` prefix docs.

### `src/embed/mod.rs`

- `create_embedder_with_config()` — `ollama:` branch: remove fallback block (the
  `#[cfg(feature = "local-embed")]` block that calls `LocalEmbedder::new(FALLBACK)`).
  Replace with `return Err(...)` containing actionable error message.
- `create_embedder_with_config()` — `openai:` branch: pass `api_key` parameter to
  `RemoteEmbedder::openai()` (new signature: `openai(model_id, api_key)`).
- Update `anyhow::bail!` error messages at lines ~229-243 that hardcode
  `local:NomicEmbedTextV15Q` → `local:AllMiniLML6V2Q`.
- Update fallback at bottom of function to use `AllMiniLML6V2Q`.

### `src/embed/local.rs`

- `parse_model()` error message: change "recommended" from `NomicEmbedTextV15Q` to
  `AllMiniLML6V2Q`. Update model list ordering to reflect new default.

### `src/embed/remote.rs`

- `RemoteEmbedder::openai()` — accept `api_key: Option<String>` parameter.
  Resolution: `api_key` param → `OPENAI_API_KEY` env var → error.

### `src/tools/workflow.rs`

- `model_options_for_hardware()` — update default recommendation to `AllMiniLML6V2Q`
  with correct values: `dims: 384`, `context_tokens: 256`.
- Bump `ONBOARDING_VERSION` (prompt surface change per CLAUDE.md rules).
- Onboarding prompt references — update to recommend URL for quality, bundled for simplicity.

### `src/prompts/onboarding_prompt.md`

- Update "Ollama is the default" language to reflect `local:AllMiniLML6V2Q` as default.
- Position Ollama/URL as upgrade options, not the primary path.

### Existing manual pages

- `docs/manual/src/configuration/embeddings.md` — update default model references
  from `NomicEmbedTextV15Q` to `AllMiniLML6V2Q`. Update "Recommended default" section.
- `docs/manual/src/configuration/embedding-backends.md` — remove "Automatic CPU Fallback"
  section (documents the Ollama fallback we're removing). Update to reflect error-on-unreachable.
- `docs/manual/src/semantic-search-guide.md` — update to reflect `AllMiniLML6V2Q` as default.

### `docs/manual/src/SUMMARY.md`

- Verify embeddings pages are correctly linked after updates.

### Tests

- Existing `ollama_*` tests in `src/embed/remote.rs` — unchanged (they test the HTTP path).
- Update `model_options_*` tests in `workflow.rs` to expect `AllMiniLML6V2Q` with 384d/256tok.
- Add test: `ollama_prefix_without_server_returns_error` — verifies no silent fallback.
- Add test: `openai_prefix_uses_config_api_key` — verifies config file key is used.
- Add test: `url_with_ollama_prefix_strips_prefix` — verifies prefix stripping with URL.
- Rename and update `create_embedder_no_url_no_prefix_defaults_to_local_nomic` →
  `create_embedder_no_url_no_prefix_defaults_to_local_allminilm` — expect `AllMiniLML6V2Q`.

## Migration

Users with existing `ollama:` configs: **no action needed** as long as Ollama is running.
If Ollama is down, they'll now get a clear error instead of silently degraded embeddings.

Users with existing indexes: the model mismatch check will trigger if they change their
config. They'll need to re-index (`index_project(force: true)`) — this is by design and
already well-handled.

New users: get `AllMiniLML6V2Q` by default. The onboarding prompt and documentation
guide them to upgrade when ready.
