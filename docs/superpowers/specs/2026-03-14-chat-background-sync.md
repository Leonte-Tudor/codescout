# Chat Background Sync & Index Status

**Date:** 2026-03-14  
**Status:** Approved  
**Scope:** `crates/chat/` + `crates/mcp-server/src/server.rs`

---

## Problem

`chat_search` currently calls `sync_space()` inline before querying the local DB. On first use
of a space, this paginates the entire Google Chat API and embeds all messages — potentially
taking 10+ minutes — while blocking the MCP tool call. There is no way to check progress or
interrupt it.

---

## Goal

- `chat_search` always returns immediately from the local SQLite DB
- Sync runs in the background, triggered by `chat_list_spaces`
- A new `chat_index_status` tool exposes per-space sync progress

---

## Design

### Trigger: `chat_list_spaces`

`chat_list_spaces` already fetches the live space list and upserts spaces into the DB. After
that, it calls `ChatService::trigger_background_sync(spaces)`. If a sync task is already
running, this is a no-op. Otherwise, it spawns a background task and returns immediately — the
tool response is unchanged.

Staleness threshold: **1 hour** (constant `SYNC_STALE_SECS = 3600` in `sync.rs`). Spaces
synced more recently than this are marked `Skipped`.

### State Model

A new `ChatSyncStatus` struct is added to `crates/chat/src/sync.rs` and held as
`Arc<Mutex<ChatSyncStatus>>` on `ChatService`:

```rust
pub struct ChatSyncStatus {
    pub running: bool,
    pub spaces: Vec<SpaceSyncEntry>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

pub struct SpaceSyncEntry {
    pub space_name: String,
    pub display_name: String,
    pub state: SpaceSyncState,
}

pub enum SpaceSyncState {
    Pending,
    Syncing { done: usize, total_estimate: Option<usize> },
    Done { synced_count: usize, last_synced_at: DateTime<Utc> },
    Failed(String),
    Skipped,  // synced_at < SYNC_STALE_SECS ago
}
```

`running: bool` is the concurrency guard — `trigger_background_sync()` checks it under the
lock before spawning. Only one sync task runs at a time.

### Background Task

`trigger_background_sync(spaces: Vec<ChatSpace>)`:

1. Acquires the lock
2. If `running == true` → returns immediately (no-op)
3. Sets `running = true`, `started_at = now()`, populates `spaces` vec (fresh spaces as
   `Skipped`, rest as `Pending`), releases lock
4. `tokio::spawn`s an async loop that iterates spaces **sequentially** (one at a time, to
   respect the Chat API rate limiter):
   - Sets entry to `Syncing { done: 0, total_estimate: None }`
   - Calls `sync_space()`, updating `done` after each page
   - Sets entry to `Done { synced_count, last_synced_at }` or `Failed(e)`
5. On loop completion: sets `running = false`, `finished_at = now()`
6. On unexpected panic/error: sets `running = false`, `error = Some(msg)`

`sync_space()` is updated to accept a progress callback
`Fn(done: usize, total_estimate: Option<usize>)` so the background task can update the
per-space `done` counter live.

### `chat_search` Simplification

Remove all sync logic from `ChatService::search()`. It becomes: embed the query → run
`hybrid_search` against the local DB → return results. If `sync_status.running == true`, a
footer note is appended to the response:

```
Note: Sync in progress (N/M spaces done) — results may be incomplete.
Call chat_index_status for details.
```

### New Tool: `chat_index_status`

MCP tool defined in `server.rs`, reads `self.chat.sync_status()` (a snapshot clone under the
lock) and returns:

```json
{
  "running": true,
  "started_at": "2026-03-14T10:00:00Z",
  "finished_at": null,
  "error": null,
  "total_spaces": 14,
  "done_count": 1,
  "skipped_count": 1,
  "pending_count": 12,
  "spaces": [
    {
      "space_name": "spaces/AAQAo8UE3io",
      "display_name": "MRV AI Use Case.",
      "state": "skipped",
      "last_synced_at": "2026-03-14T09:58:00Z"
    },
    {
      "space_name": "spaces/AAQAP00OIGo",
      "display_name": "Engineering",
      "state": "syncing",
      "done": 42,
      "total_estimate": null
    },
    {
      "space_name": "spaces/AAAArt3oZPg",
      "display_name": "AI Innovation Guild",
      "state": "pending"
    },
    {
      "space_name": "spaces/AAQAody1OP0",
      "display_name": "Certificates MRV",
      "state": "done",
      "synced_count": 187,
      "last_synced_at": "2026-03-14T10:01:30Z"
    }
  ]
}
```

---

## Files Changed

| File | Change |
|------|--------|
| `crates/chat/src/sync.rs` | Add `ChatSyncStatus`, `SpaceSyncEntry`, `SpaceSyncState`; add progress callback to `sync_space` |
| `crates/chat/src/lib.rs` | Add `sync_status: Arc<Mutex<ChatSyncStatus>>` to `ChatService`; add `trigger_background_sync()`, `sync_status()` methods; simplify `search()` |
| `crates/mcp-server/src/server.rs` | Simplify `chat_search` handler; add `chat_index_status` tool |

---

## Non-Goals

- Parallel space syncing (sequential is sufficient and rate-limit-safe)
- Startup sync (triggered by `chat_list_spaces` instead, avoids boot-time auth issues)
- Configurable staleness threshold via config file (constant is sufficient for now)
- Push notifications when sync completes (poll via `chat_index_status`)
