# LSP Startup Statistics — Design Spec

**Date:** 2026-03-15  
**Status:** Approved  
**Branch:** experiments

## Overview

Add instrumentation to the LSP cold-start path that records startup events to the existing
`usage.db` SQLite database and surfaces them in the project dashboard. Two durations are
captured per event: the LSP initialize handshake (always available) and the first real LSP
response (the latency the user actually perceives).

## Motivation

The idle TTL eviction feature (commit `5d8f3ed`) kills LSP clients after 20 minutes of idle
time. When the LSP is evicted and then immediately needed, the cold-start latency is visible
to the user but not observable in any structured way. This feature makes it observable and
trackable across LSP version upgrades (e.g., today's rust-analyzer 1.89→1.94 update).

## Data Model

New table added to `.codescout/usage.db` in `open_db`'s `CREATE TABLE IF NOT EXISTS` block:

```sql
CREATE TABLE IF NOT EXISTS lsp_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    language            TEXT NOT NULL,
    started_at          TEXT NOT NULL DEFAULT (datetime('now')),
    reason              TEXT NOT NULL,
    handshake_ms        INTEGER NOT NULL,
    first_response_ms   INTEGER
);
```

**`reason` values:** `new_session` | `idle_evicted` | `lru_evicted` | `crashed`

- `first_response_ms` is NULL until a tool reports back; may stay NULL if no tool call
  follows the cold start within the same process lifetime.

## Write Path

### `src/usage/db.rs`

Two new write functions:

```rust
pub fn write_lsp_event(conn: &Connection, language: &str, reason: &str, handshake_ms: i64) -> Result<i64>
pub fn update_lsp_first_response(conn: &Connection, rowid: i64, first_response_ms: i64) -> Result<()>
```

`write_lsp_event` returns the inserted rowid for use by the two-phase write.

### `src/lsp/manager.rs`

**New field on `LspManager`:**

```rust
pending_first_response: Mutex<HashMap<LspKey, (i64, Instant)>>
//                                             rowid  cold-start began
```

**`do_start`** receives a `reason: &str` parameter. After `LspClient::start()` succeeds:

1. Opens `usage.db` best-effort (skips on failure — never blocks startup)
2. Calls `write_lsp_event` → gets rowid
3. Inserts `(rowid, Instant::now())` into `pending_first_response`
4. Logs `"LSP initialized in {handshake_ms}ms (reason: {reason})"` at INFO level

**Reason inference** in `get_or_start`:

| Situation | Reason passed to `do_start` |
|---|---|
| No existing client | `new_session` |
| Dead client present (`is_alive() == false`) | `crashed` |
| Called after LRU eviction | `lru_evicted` |
| Called after idle TTL eviction | `idle_evicted` |

LRU and idle eviction paths set a `pending_reason: Mutex<HashMap<LspKey, String>>`
entry before evicting; `do_start` consumes it (defaulting to `"new_session"` if absent).
Using `String` (not `&'static str`) keeps the type consistent with `reason: &str` on
`do_start` and avoids a forced refactor if a future caller supplies a dynamic reason.

**Concurrency:** `get_or_start` already serializes concurrent calls for the same `LspKey`
via the `starting` watch-channel barrier — the first caller becomes the starter, all others
wait for it to finish. This guarantees `do_start` is never called concurrently for the same
key, so there is no race on `pending_first_response` insertion.

## Read Path

### `src/usage/db.rs`

One new read function (moved here from Write Path):

```rust
pub fn query_lsp_stats(conn: &Connection, window: &str) -> Result<LspStats>
```

Returns `LspStats { by_language: Vec<LspLanguageStats>, recent: Vec<LspEvent> }` where
`LspLanguageStats` holds avg/p95 for both durations and reason counts, and `LspEvent` is a
flat row for the recent list.

### `LspProvider` trait

New method with a default no-op implementation (so `MockLspProvider` requires no changes):

```rust
async fn record_first_response(&self, _language: &str, _workspace_root: &Path, _elapsed_ms: i64) {}
```

`LspManager` overrides this with the real logic:
1. Builds `LspKey` via the same `LspKey::new(language, workspace_root)` constructor used in
   `do_start` — this is the single source of truth; tool call sites must pass the same
   `language` and `workspace_root` they use for `get_or_start` to ensure key alignment.
2. Removes from `pending_first_response` (first caller wins; subsequent calls are silent
   no-ops since the entry is consumed).
3. If entry found: calls `update_lsp_first_response` via `tokio::task::spawn_blocking`,
   matching the pattern used elsewhere in the codebase for SQLite writes inside async
   context (rusqlite is synchronous; blocking inside an async task would stall the executor).
4. Logs `"LSP first response in {elapsed_ms}ms"` at DEBUG level.

### Tool call sites

Five tools wrap their first LSP call with timing and call `record_first_response` after:

- `list_symbols`
- `find_symbol`
- `goto_definition`
- `hover`
- `find_references`

Pattern (best-effort, fire-and-forget):

```rust
let lsp_start = Instant::now();
let result = client.document_symbols(...).await;
let _ = ctx.lsp.record_first_response(language, &root, lsp_start.elapsed().as_millis() as i64).await;
```

Since `pending_first_response` is consumed on first call, only the first of these tools to
execute after a cold start actually writes; the rest are silent no-ops.

## Dashboard

### New API endpoint

`GET /api/lsp?window=30d` in `src/dashboard/api/lsp.rs`:

```json
{
  "available": true,
  "window": "30d",
  "by_language": [
    {
      "language": "rust",
      "starts": 12,
      "reasons": { "new_session": 4, "idle_evicted": 7, "lru_evicted": 0, "crashed": 1 },
      "avg_handshake_ms": 820,
      "p95_handshake_ms": 1400,
      "avg_first_response_ms": 8200,
      "p95_first_response_ms": 14000
    }
  ],
  "recent": [
    {
      "language": "rust",
      "started_at": "2026-03-15 14:23:01",
      "reason": "idle_evicted",
      "handshake_ms": 950,
      "first_response_ms": 9100
    }
  ]
}
```

`recent` returns the last 20 events ordered by `started_at DESC`.

### Dashboard UI (`src/dashboard/static/`)

New "LSP Startup" section below the existing tool stats section, sharing the same time-window
selector. Added to the `Promise.all` in `refreshStats()`.

**Aggregate table** — one row per language:

| Language | Starts | Reasons | Avg handshake | p95 handshake | Avg first resp | p95 first resp |
|---|---|---|---|---|---|---|
| rust | 12 | 4 new · 7 evicted · 1 crash | 820ms | 1.4s | 8.2s | 14s |

Reason counts shown as small inline badges. `first_response_ms` columns show `—` when all
values are NULL for that language.

**Recent events list** — last 20 rows (matching `recent` in the API response), compact
one-liner format:

```
[rust] idle_evicted · handshake 950ms · first response 9.1s · 14 min ago
```

Registered in `src/dashboard/mod.rs` route table alongside existing API handlers.

## Testing

### `src/usage/db.rs`

| Test | What it verifies |
|---|---|
| `write_lsp_event_returns_rowid` | Insert succeeds, rowid > 0 |
| `update_lsp_first_response_fills_null` | Write then update, assert `first_response_ms` set |
| `query_lsp_stats_aggregates_correctly` | Multiple events, assert avg/p95 per language |
| `query_lsp_stats_window_excludes_old_rows` | Old rows outside window not counted |

### `src/lsp/manager.rs`

| Test | What it verifies |
|---|---|
| `do_start_records_lsp_event` | Row written to DB after cold start |
| `record_first_response_consumes_pending` | Call twice for same key; only one DB write |
| `record_first_response_noop_without_pending` | No pending entry → no panic, no error |
| `do_start_reason_crashed_when_dead_client_present` | Dead client in map → reason is `crashed` |
| `do_start_reason_evicted_consumes_pending_reason` | `pending_reason` entry set before eviction → consumed by next `do_start`, reason recorded correctly |

`MockLspProvider` requires no changes — the new `record_first_response` trait method has a
default no-op implementation.

## Files Changed

| File | Change |
|---|---|
| `src/usage/db.rs` | Add `lsp_events` table, 3 new functions, 4 new tests |
| `src/usage/mod.rs` | No changes — tools call `db::write_lsp_event` directly via the same pattern as existing recording |
| `src/lsp/manager.rs` | Add `pending_first_response` + `pending_reason` fields, `reason` param on `do_start`, `record_first_response` method, 4 new tests |
| `src/lsp/ops.rs` | Add default-impl `record_first_response` to `LspProvider` trait |
| `src/tools/symbols.rs` | Wrap first LSP call in `list_symbols` + `find_symbol` |
| `src/tools/nav.rs` | Wrap first LSP call in `goto_definition` + `hover` + `find_references` |
| `src/dashboard/api/lsp.rs` | New file: `get_lsp` handler |
| `src/dashboard/api/mod.rs` | Register `get_lsp` |
| `src/dashboard/mod.rs` | Add `/api/lsp` route |
| `src/dashboard/static/dashboard.js` | Add LSP Startup section to `refreshStats()` |
| `src/dashboard/static/index.html` | Add LSP Startup section HTML skeleton |
