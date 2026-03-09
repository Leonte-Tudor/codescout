# Design: Trim `project_status` Output

**Date:** 2026-03-09  
**Status:** Approved  
**Scope:** Remove noise from `project_status` — replace full config blob with flat essential fields, collapse index section to a summary with a hint to `index_status`.

## Problem

`project_status` returns a full `config` object (security rules, memory thresholds, ignored_paths, encoding, tool_timeout_secs...) and a verbose `index` section that duplicates `index_status`. Most of this is noise at session start — an agent just needs to orient, not audit the full configuration.

## Goal

`project_status` becomes a **compact session overview**: languages, embedding model, library count, index health at a glance, memory staleness. Details delegated to `index_status`.

## Design

### New output shape

```json
{
  "project_root": "/home/.../code-explorer",
  "languages": ["rust", "typescript", "python"],
  "embeddings_model": "ollama:mxbai-embed-large",
  "libraries": { "count": 0, "indexed": 0 },
  "index": {
    "status": "up_to_date",
    "files": 289,
    "chunks": 11224,
    "last_updated": "2026-03-09 12:00 UTC",
    "hint": "Call index_status() for model info, by_source breakdown, drift, and progress details."
  },
  "memory_staleness": { "stale": [...], "fresh": [...], "untracked": [...] }
}
```

### `index.status` values

| Value | Condition |
|---|---|
| `"running"` | `IndexingState::Running` — also includes `done`, `total`, `eta_secs` |
| `"up_to_date"` | Index exists + git_sync not stale |
| `"behind"` | Index exists + git_sync stale |
| `"not_indexed"` | No DB file |

When `status = "running"`:
```json
"index": {
  "status": "running",
  "done": 23,
  "total": 87,
  "eta_secs": 45,
  "hint": "Call index_status() for detailed breakdown."
}
```

When `status = "not_indexed"`:
```json
"index": { "status": "not_indexed", "hint": "Run index_project() to build the index." }
```

### What's removed

| Field | Where it goes |
|---|---|
| `config` (full blob) | Replaced by flat `languages` + `embeddings_model` |
| `index.drift` | `index_status(threshold=...)` |
| `index.git_sync` object | Collapsed into `index.status: "behind"` |
| `index.model` | `index_status()` |

### What `index_status` already covers (unchanged)

`index_status` remains the authoritative source for: `configured_model`, `indexed_with_model`, `embedding_count`, `db_path`, `by_source`, `last_indexed_commit`, full `git_sync` details, `drift`, and the `indexing` running state.

## Files Changed

| File | Change |
|---|---|
| `src/tools/config.rs` | `ProjectStatus::call` — new output shape |
| `src/tools/config.rs` tests | Update assertions for new shape |

## Out of Scope

- Changes to `index_status`
- Changes to `server.rs`, `agent.rs`, or any other file
- The `threshold`/`path` drift params on `project_status` are removed (delegate to `index_status`)
