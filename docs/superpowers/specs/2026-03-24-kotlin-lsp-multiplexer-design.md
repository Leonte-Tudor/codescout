# Kotlin LSP Multiplexer Design

**Date:** 2026-03-24
**Status:** Reviewed (v2 — post spec review fixes)
**Branch:** experiments

## Problem

Multiple codescout instances targeting the same Kotlin project cause severe degradation:

1. **kotlin-lsp single-session limitation**: JetBrains' kotlin-lsp (IntelliJ Platform) allows only one LSP process per workspace directory. Our `--system-path` per-instance isolation avoids the hard crash but both JVMs still load the full Gradle project model (~1-2GB each).

2. **Gradle daemon contention**: `--system-path` only isolates IntelliJ's `.app.lock` — it does NOT isolate the Gradle daemon. Two kotlin-lsp instances fight over `~/.gradle/caches/journal-1/journal-1.lock`, causing 120s+ timeouts.

3. **Resource waste**: Two kotlin-lsp JVMs consume ~3-4GB combined RAM with duplicate project models, indexes, and Gradle caches.

4. **Degraded responses**: Under resource pressure, kotlin-lsp returns empty or partial document symbol results, surfacing as misleading "symbol not found" errors.

## Solution

A detached **LSP multiplexer process** (`codescout mux`) that manages a single kotlin-lsp instance and allows multiple codescout instances to share it via a Unix socket.

### Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ codescout-A  │     │ codescout-B  │     │ codescout-C  │
│ (LspClient)  │     │ (LspClient)  │     │ (LspClient)  │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │ Unix socket        │                     │
       └────────────┬───────┘─────────────────────┘
                    │
          ┌─────────▼──────────┐
          │  codescout mux     │  (detached process)
          │  - holds flock     │
          │  - ID remapping    │
          │  - doc state dedup │
          │  - idle timeout    │
          └─────────┬──────────┘
                    │ stdio
          ┌─────────▼──────────┐
          │   kotlin-lsp       │
          │  (single JVM)      │
          └────────────────────┘
```

**Three actors:**
- **codescout instances** — connect as clients via socket, speak normal LSP
- **codescout mux** — routes messages, deduplicates document state, manages kotlin-lsp lifecycle
- **kotlin-lsp** — the actual language server, unaware of multiplexing

### Key Design Decisions

1. **Detached subprocess, not embedded**: The mux process outlives any single codescout session. kotlin-lsp stays warm across codescout restarts — no 8-15s cold starts after the first session. The mux exits on idle timeout (300s) after the last client disconnects.

2. **flock-based ownership**: The mux process holds an exclusive `flock` on a lock file for its entire lifetime. When it dies (any cause including SIGKILL), the OS releases the lock. Next codescout instance detects this via `try_lock_exclusive()` and spawns a fresh mux. No PID files, no heartbeats, no race conditions.

3. **LSP-server-agnostic**: The mux speaks standard LSP framing on both sides (socket ↔ stdio). It could multiplex any LSP server, not just kotlin-lsp. Only Kotlin opts in today via a `mux: true` flag in `LspServerConfig`.

4. **Gradle isolation included**: As a quick-win, the kotlin-lsp spawn adds `GRADLE_USER_HOME` isolation regardless of the mux. This fixes Gradle daemon contention even if two mux instances somehow co-exist.

## Components

### Component 1: Ownership Protocol

Determines whether to spawn a new mux or connect to an existing one. Lives in `LspManager::get_or_start()`.

**Files:**
- Lock: `/tmp/codescout-kotlin-mux-<workspace-hash>.lock`
- Socket: `/tmp/codescout-kotlin-mux-<workspace-hash>.sock`
- Hash: stable hash of canonical workspace root path

**Flow:**

```
get_or_start("kotlin", workspace_root)
  → cache check (existing behavior)
  → config.mux == true?
    ├─ No  → LspClient::start(config) as before
    └─ Yes → try flock (LOCK_EX | LOCK_NB)
               ├─ Acquired → spawn_mux_process()
               │   ├─ Spawn `codescout mux` with stdout pipe
               │   ├─ Release flock immediately (mux will acquire its own)
               │   ├─ Wait for "ready\n" on stdout (up to 120s for kotlin cold start)
               │   ├─ If mux exits non-zero (lost race for flock) → retry from flock step
               │   └─ LspClient::connect(socket_path)
               │
               └─ Blocked → mux already running
                   ├─ LspClient::connect(socket_path)
                   ├─ If ECONNREFUSED → sleep 100ms, retry from flock (mux died between flock and bind)
                   └─ Return LspClient
```

**Flock handshake**: The spawning codescout releases the flock *before* waiting for "ready". The mux acquires the flock on its own. If a third process races in and acquires the flock first, the mux detects it cannot acquire the lock, exits non-zero, and the spawning codescout retries from the top. This eliminates the deadlock where parent and child both try to hold `LOCK_EX` on the same file.

**Dependencies:** `fs2` or `fd-lock` crate for cross-platform file locking.
### Component 2: Mux Process

New subcommand: `codescout mux`. Single-purpose, ~500 lines.

**Invocation:**

```
codescout mux \
  --socket /tmp/codescout-kotlin-mux-<hash>.sock \
  --lock /tmp/codescout-kotlin-mux-<hash>.lock \
  --idle-timeout 300 \
  -- kotlin-lsp --stdio --system-path=...
```

Everything after `--` is the LSP server command.

**Startup sequence:**

1. Acquire flock on lock file
2. Spawn kotlin-lsp as child process with stdio pipes
3. Perform LSP `initialize` handshake with kotlin-lsp, cache `InitializeResult`
4. Bind Unix socket listener
5. Write `"ready\n"` to stdout (signals parent), then close stdout
6. Enter event loop

**Event loop — 4 concurrent tasks:**

| Task | Role |
|------|------|
| Accept loop | Accept new client connections on socket |
| Server stdout reader | Read LSP messages from kotlin-lsp, route to client(s) |
| Client readers (per client) | Read LSP messages from clients, remap IDs, forward to kotlin-lsp |
| Idle watchdog | Every 10s: if 0 clients for > idle-timeout, exit |

#### ID Remapping

Each client gets a short tag (`a`, `b`, `c`, ...) on connect.

```
Client A sends:  {"id": 1, ...}    → Mux forwards: {"id": "a:1", ...}
Client B sends:  {"id": 1, ...}    → Mux forwards: {"id": "b:1", ...}

kotlin-lsp responds: {"id": "a:1", ...} → Mux routes to A: {"id": 1, ...}
kotlin-lsp responds: {"id": "b:1", ...} → Mux routes to B: {"id": 1, ...}
```

Note: LSP allows both integer and string IDs. Most LSP servers handle string IDs fine. If kotlin-lsp ever rejects string IDs, we can switch to a numeric scheme (e.g., client_index * 1_000_000 + original_id).

#### Document State Dedup

Tracks `HashMap<Uri, (HashSet<ClientTag>, u64)>` — which clients have each file open, plus a monotonic version counter per URI.

| Event | Action |
|-------|--------|
| Client A `didOpen(foo.kt)` | Forward to kotlin-lsp. Record: `foo.kt → ({A}, version=1)` |
| Client B `didOpen(foo.kt)` | Suppress (don't forward). Record: `foo.kt → ({A, B}, version=1)` |
| Client A `didClose(foo.kt)` | Suppress (B still open). Record: `foo.kt → ({B}, version=1)` |
| Client B `didClose(foo.kt)` | Forward to kotlin-lsp. Record: `foo.kt → ({}, -)` |
| Any client `didChange(foo.kt, v=?)` | Rewrite version to next monotonic value, forward. E.g., `version=2` |

**Version rewriting**: The mux maintains a single monotonic version counter per URI. All `didChange` notifications have their `version` field rewritten to the next value in the sequence before forwarding to kotlin-lsp. This prevents version conflicts when two clients edit the same file. Note: concurrent edits to the same file from different clients produce undefined document state — this is acceptable since two agents editing the same file simultaneously is already a bug.
#### Notification Routing (kotlin-lsp → clients)

| Type | Routing |
|------|---------|
| `textDocument/publishDiagnostics` | Broadcast to all |
| `window/logMessage`, `window/showMessage` | Broadcast to all |
| `$/progress` | Broadcast to all |

#### Server Request Routing (kotlin-lsp → mux)

| Request | Handling |
|---------|----------|
| `client/registerCapability` | Broadcast to all + cache for future clients |
| `workspace/configuration` | Forward to one client, return response |
| `workspace/applyEdit` | Forward to the client that holds the edit lock (see below) |
| Others | Auto-respond with `null` |

**Edit lock for `workspace/applyEdit`**: Operations that can trigger server-initiated edits (rename, code actions) are serialized through a per-mux lock. When a client sends a rename request, the mux records that client's tag as the "edit owner". Any `workspace/applyEdit` request from kotlin-lsp is routed to the edit owner. The lock is released when the rename response is sent back to the client. If two clients try to rename concurrently, the second blocks until the first completes. This is safe — renames are rare, human-initiated operations.
#### Client Lifecycle

**Socket handshake protocol**: The mux uses an out-of-band handshake, not LSP `initialize`.

1. Client connects to socket
2. Mux immediately sends a framed JSON message: `{"type": "init", "result": <cached InitializeResult>, "capabilities_registered": [<cached registerCapability requests>]}`
3. Client reads this, populates its `capabilities` field
4. Client begins sending normal LSP requests (no `initialize` or `initialized` needed)

This means `LspClient::connect()` does NOT call `self.initialize()`. The mux has already performed the handshake with kotlin-lsp.

- **Connect**: Assign tag, send init message with cached state
- **Disconnect**: Clean up doc state (close files only this client had open), decrement client count
- **Reconnect**: No special handling — treated as a new client
#### Shutdown

| Trigger | Action |
|---------|--------|
| Idle timeout (300s, 0 clients) | Send `shutdown` + `exit` to kotlin-lsp, unlink socket, release flock, exit |
| kotlin-lsp crashes (EOF on stdout) | Log error, unlink socket, release flock, exit |
| Mux receives SIGTERM | Graceful shutdown: disconnect clients, shutdown kotlin-lsp, cleanup, exit |
| Mux killed (SIGKILL) | OS releases flock. Socket file lingers. Next codescout detects via flock and cleans up |

#### Logging

The mux process logs to a file. Preferred location: `.codescout/mux-kotlin-<hash>.log` in the workspace root. If `.codescout/` does not exist, fall back to `/tmp/codescout-mux-kotlin-<hash>.log`. Rotation: keep 2 most recent. Includes: client connect/disconnect, kotlin-lsp lifecycle events, errors.
### Component 3: LspClient Socket Transport

New transport variant in `LspClient` to connect via socket instead of spawning a process.

**Writer type generalization**: The current `writer` field is `Arc<Mutex<ChildStdin>>`. This must be generalized to `Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>` so both `ChildStdin` (process mode) and `OwnedWriteHalf` (socket mode) can be stored. All methods that use the writer (`request_with_timeout`, `notify`, `initialize`, `did_open`, `did_change`, `did_close`, `shutdown`) already call `transport::write_message(&mut *writer, &msg)` which only requires `AsyncWriteExt + Unpin` — so the change is type-level, not logic-level.

**Transport enum:**

```rust
enum LspTransport {
    Process {
        child_pid: Option<u32>,
        reader_handle: StdMutex<Option<JoinHandle<()>>>,
    },
    Socket {
        socket_path: PathBuf,
    },
}
```

**New constructor:**

```rust
impl LspClient {
    pub async fn connect(socket_path: &Path, workspace_root: PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await?;
        let (read_half, write_half) = stream.into_split();

        // Read the mux init message (cached InitializeResult + registered capabilities)
        // Populate self.capabilities from the init message
        // Do NOT call self.initialize() — mux already handled the handshake

        // Same reader task, pending map, writer mutex as start()
        // No child process, no stderr reader, no kill_on_drop
    }
}
```

**Behavior differences by transport:**

| Operation | Process | Socket |
|-----------|---------|--------|
| `start()` / `connect()` | Spawns child, pipes stdio | Connects to socket, reads init message |
| `writer` type | `Box<dyn AsyncWrite + ..>` wrapping `ChildStdin` | `Box<dyn AsyncWrite + ..>` wrapping `OwnedWriteHalf` |
| `initialize()` | Sends LSP initialize handshake | Skipped — mux sends cached result on connect |
| `open_files` tracking | Local HashMap (current behavior) | Skipped — mux handles document state dedup |
| `shutdown()` | Send shutdown+exit, wait for child | Send shutdown (mux intercepts, disconnects us) |
| `is_alive()` | Check `alive` AtomicBool | Same (reader task sets false on socket EOF) |
| `Drop` | `kill_on_drop` on child | Close socket (mux detects, cleans doc state) |
| Stderr reading | Spawns stderr reader task | Not applicable (mux handles stderr) |

**Methods requiring transport-aware changes:**
- `request_with_timeout` — writer type only (no logic change)
- `notify` — writer type only
- `shutdown` — conditional: process sends exit notification, socket just disconnects
- `Drop` — conditional: process kills child, socket closes connection
- `did_open` / `did_change` / `did_close` — skip local `open_files` tracking in socket mode
### Component 4: LspServerConfig Extension

```rust
pub struct LspServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub workspace_root: PathBuf,
    pub init_timeout: Option<Duration>,
    pub mux: bool,  // NEW — enable multiplexer
}
```

`default_config("kotlin", ...)` sets `mux: true`. All other languages: `mux: false`.

Future: if jdtls (Java) ever needs sharing, flip one flag.

### Quick-Win: Gradle Isolation

Regardless of the mux, the kotlin-lsp spawn environment includes:

```rust
"kotlin" => {
    let system_dir = temp_dir().join(format!("codescout-mux-kotlin-lsp"));
    let gradle_home = temp_dir().join(format!("codescout-mux-gradle"));
    // Set GRADLE_USER_HOME in child environment
    ...
}
```

This eliminates Gradle daemon cache lock contention. With the mux, there's only one kotlin-lsp, so only one `GRADLE_USER_HOME` is needed (no per-PID suffix).

## Cross-Platform Considerations

| Platform | Socket mechanism | Stale cleanup |
|----------|-----------------|---------------|
| Linux | Unix domain socket (or abstract socket for auto-cleanup) | flock released on death |
| macOS | Unix domain socket | flock released on death |
| Windows | Named pipe via `interprocess` crate | `LockFileEx` released on death |

The `interprocess` crate provides a unified `local_socket` API across platforms. Consider it if Windows support is needed. For now, Unix sockets + flock covers Linux and macOS.

## Error Handling

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Mux process crashes | LspClient reader EOF → `alive = false` | Next tool call: `get_or_start` evicts dead client, acquires flock, spawns fresh mux |
| kotlin-lsp crashes | Mux detects EOF on child stdout | Mux exits (unlinks socket, releases flock). Clients get EOF. Next call spawns fresh mux |
| Socket connection refused | `LspClient::connect()` returns error | Retry flock acquisition (mux died between flock and bind) |
| Flock permanently stuck | Should not happen (OS releases on death) | Circuit breaker trips after repeated failures |
| Mux OOM-killed | OS releases flock + socket FDs | Same as mux crash recovery |

## Scope

| Component | New code | Modifies |
|-----------|----------|----------|
| Ownership protocol | ~80 lines, `src/lsp/mux.rs` | — |
| Mux process | ~500 lines, `src/mux/` + CLI subcommand | `src/main.rs` |
| LspClient socket transport | ~150 lines, `LspClient::connect()` | `src/lsp/client.rs` (~150 lines: writer type generalization, transport enum, conditional shutdown/Drop/open_files) |
| LspManager integration | ~80 lines | `src/lsp/manager.rs` (~30 lines), `src/lsp/servers/mod.rs` (~10 lines) |
| Gradle isolation | ~5 lines | `src/lsp/servers/mod.rs` |
| **Total** | **~815 lines new** | **~190 lines modified** |

**New dependencies:** `fs2` or `fd-lock` (file locking). No other external dependencies — Unix sockets via `tokio::net::UnixListener`.
## Testing Strategy

1. **Unit tests**: ID remapping, document state dedup, flock acquisition/release
2. **Integration test**: Spawn mux, connect two mock clients, verify request routing and doc state dedup
3. **Manual test**: Two codescout instances on kotlin-library fixture, verify `find_references` works from both simultaneously
4. **Crash recovery test**: Kill mux process, verify next codescout session auto-recovers

## Not In Scope

- Multi-language mux (only Kotlin for now; `mux: bool` flag makes future extension trivial)
- Windows named pipe support (Unix sockets cover Linux + macOS; add `interprocess` later if needed)
- lspmux integration (external daemon approach rejected — see research notes)
- Replacing JetBrains kotlin-lsp with community alternatives (all too immature)

## Research References

- [lspmux](https://codeberg.org/p2502/lspmux) — Rust LSP multiplexer, reference implementation for ID remapping and doc state dedup
- [LSP multi-client issue #1160](https://github.com/microsoft/language-server-protocol/issues/1160) — protocol-level discussion
- [kotlin-lsp](https://github.com/Kotlin/kotlin-lsp) — JetBrains' official Kotlin LSP, pre-alpha
- [Gradle daemon docs](https://docs.gradle.org/current/userguide/gradle_daemon.html) — daemon sharing and isolation
- `fs2` / `fd-lock` crates — cross-platform file locking
