# index_project Progress & ETA Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Surface per-file embedding progress and a heuristic ETA through `index_status` when `index_project` is running in the background.

**Architecture:** Enrich `IndexingState::Running` with `done/total/eta_secs` fields; pass a lightweight callback into `build_index` that updates the shared mutex after each file is embedded; update `index_status` to render the new fields.

**Tech Stack:** Rust, `std::time::Instant`, `std::sync::Mutex<IndexingState>`, existing `JoinSet` drain loop in `src/embed/index.rs`.

**Design doc:** `docs/plans/2026-03-09-index-progress-design.md`

---

### Task 1: Extend `IndexingState::Running` and fix all use sites

**Files:**
- Modify: `src/agent.rs:19-31`
- Modify: `src/tools/semantic.rs` (3 sites: L280, L286, L522)

---

**Step 1: Write the failing test**

Add this test to the `tests` module in `src/tools/semantic.rs` (after the existing `index_status_with_data` test, around L793):

```rust
#[tokio::test]
async fn index_status_shows_running_progress() {
    use crate::agent::IndexingState;
    let (dir, ctx) = project_ctx().await;
    // Create the DB so index_status doesn't early-return "no index"
    let conn = crate::embed::index::open_db(dir.path()).unwrap();
    drop(conn);

    // Simulate mid-run state
    {
        let mut state = ctx.agent.indexing.lock().unwrap();
        *state = IndexingState::Running {
            done: 10,
            total: 50,
            eta_secs: Some(20),
        };
    }

    let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
    let indexing = &result["indexing"];
    assert_eq!(indexing["status"], "running");
    assert_eq!(indexing["done"], 10);
    assert_eq!(indexing["total"], 50);
    assert_eq!(indexing["eta_secs"], 20);
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test index_status_shows_running_progress 2>&1 | tail -20
```

Expected: compile error — `IndexingState::Running` does not take fields.

**Step 3: Update `IndexingState::Running` to struct variant**

In `src/agent.rs`, replace:

```rust
    Running,
```

with:

```rust
    Running {
        done: usize,
        total: usize,
        eta_secs: Option<u64>,
    },
```

**Step 4: Fix `matches!` guard in `index_project::call` (semantic.rs ~L280)**

Replace:

```rust
            if matches!(*state, IndexingState::Running) {
```

with:

```rust
            if matches!(*state, IndexingState::Running { .. }) {
```

**Step 5: Fix initial state assignment in `index_project::call` (semantic.rs ~L286)**

Replace:

```rust
            *state = IndexingState::Running;
```

with:

```rust
            *state = IndexingState::Running {
                done: 0,
                total: 0,
                eta_secs: None,
            };
```

**Step 6: Fix `index_status` render arm (semantic.rs ~L522)**

Replace:

```rust
                IndexingState::Running => {
                    result["indexing"] = json!("running");
                }
```

with:

```rust
                IndexingState::Running { done, total, eta_secs } => {
                    result["indexing"] = json!({
                        "status": "running",
                        "done": done,
                        "total": total,
                        "eta_secs": eta_secs,
                    });
                }
```

**Step 7: Run tests to verify they pass**

```bash
cargo test index_status 2>&1 | tail -20
```

Expected: all `index_status_*` tests pass.

**Step 8: Commit**

```bash
git add src/agent.rs src/tools/semantic.rs
git commit -m "feat: extend IndexingState::Running with done/total/eta_secs"
```

---

### Task 2: Add `progress_cb` to `build_index` and fix call sites

**Files:**
- Modify: `src/embed/index.rs` (function signature + Phase 2 loop)
- Modify: `src/main.rs:95` (pass `None`)
- Modify: `src/tools/semantic.rs:296` (pass `None` temporarily)

---

**Step 1: Add `progress_cb` parameter to `build_index`**

In `src/embed/index.rs`, change the function signature from:

```rust
pub async fn build_index(project_root: &Path, force: bool) -> Result<IndexReport> {
```

to:

```rust
pub async fn build_index(
    project_root: &Path,
    force: bool,
    progress_cb: Option<Box<dyn Fn(usize, usize, Option<u64>) + Send>>,
) -> Result<IndexReport> {
```

**Step 2: Capture `total_to_embed` and `embed_start` before Phase 2**

In `build_index`, just before the `// ── Phase 2` comment, find the line where `tasks` is created. After all `tasks.spawn(...)` calls complete and before the drain loop, add:

```rust
    let total_to_embed = works.len();
    let embed_start = std::time::Instant::now();
    let mut embed_done = 0usize;
```

**Step 3: Fire callback in the `join_next` drain loop**

The current drain loop is:

```rust
    let mut results: Vec<FileResult> = Vec::new();
    while let Some(res) = tasks.join_next().await {
        results.push(res.map_err(|e| anyhow::anyhow!(e))??);
    }
```

Replace with:

```rust
    let mut results: Vec<FileResult> = Vec::new();
    while let Some(res) = tasks.join_next().await {
        results.push(res.map_err(|e| anyhow::anyhow!(e))??);
        embed_done += 1;
        if let Some(cb) = &progress_cb {
            let remaining = total_to_embed - embed_done;
            let eta_secs = (embed_done > 0 && remaining > 0).then(|| {
                let elapsed = embed_start.elapsed().as_secs_f64();
                (elapsed / embed_done as f64 * remaining as f64) as u64
            });
            cb(embed_done, total_to_embed, eta_secs);
        }
    }
```

**Step 4: Fix `src/main.rs` call site**

Change:

```rust
            codescout::embed::index::build_index(&root, force).await?;
```

to:

```rust
            codescout::embed::index::build_index(&root, force, None).await?;
```

**Step 5: Fix `src/tools/semantic.rs` call site temporarily**

Change (around L296):

```rust
            let result = crate::embed::index::build_index(&root, force).await;
```

to:

```rust
            let result = crate::embed::index::build_index(&root, force, None).await;
```

(This `None` will be replaced with the real callback in Task 3.)

**Step 6: Run tests**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass. No behavior change yet — callback is wired up in the loop but nobody passes one.

**Step 7: Commit**

```bash
git add src/embed/index.rs src/main.rs src/tools/semantic.rs
git commit -m "feat: add progress_cb to build_index with per-file ETA"
```

---

### Task 3: Wire callback in `index_project::call`

**Files:**
- Modify: `src/tools/semantic.rs` (`index_project::call` + `index_project_call_accepts_progress_none` test)

---

**Step 1: Replace `None` with a real callback in `index_project::call`**

In `src/tools/semantic.rs`, find the block that spawns the background task (around L289–310). Replace the temporary `None` call with a callback that updates the shared state.

The full replacement area — from just after the initial `*state = IndexingState::Running { ... }` block to the `tokio::spawn` call — should look like this:

```rust
        let state_arc = ctx.agent.indexing.clone();
        let progress = ctx.progress.clone();
        // Signal start immediately (step 0 = initializing).
        if let Some(p) = &progress {
            p.report(0, None).await;
        }

        let state_arc_cb = ctx.agent.indexing.clone();
        let progress_cb: Option<Box<dyn Fn(usize, usize, Option<u64>) + Send>> =
            Some(Box::new(move |done, total, eta_secs| {
                let mut s = state_arc_cb.lock().unwrap_or_else(|e| e.into_inner());
                *s = IndexingState::Running { done, total, eta_secs };
            }));

        tokio::spawn(async move {
            let result = crate::embed::index::build_index(&root, force, progress_cb).await;
            // ... rest of spawn body unchanged
```

Everything inside the `tokio::spawn` body after `build_index` (the `stats` collection, the `Done`/`Failed` state set, and the `p.report(1, Some(1))` call) remains identical.

**Step 2: Update `index_project_call_accepts_progress_none` test**

The current test is a no-op compile check. Replace it with a test that verifies the initial `Running` state is set correctly when `index_project` is invoked:

```rust
#[tokio::test]
async fn index_project_sets_initial_running_state() {
    use crate::agent::IndexingState;
    // Call index_project on a project with no embedder configured.
    // It will start and immediately fail in the background, but the
    // initial Running state should be set before the spawn.
    let (_dir, ctx) = project_ctx().await;
    let _ = IndexProject.call(json!({}), &ctx).await;

    // Give the background task a moment to start (it may succeed or fail,
    // but the initial state set is synchronous before the spawn).
    // We just verify the state was Running at some point by checking it
    // was not left as Idle:
    let state = ctx.agent.indexing.lock().unwrap().clone();
    // State is either Running (still in progress) or Done/Failed (finished fast)
    // — either way, it was never left Idle after a call.
    assert!(!matches!(state, IndexingState::Idle));
}
```

**Step 3: Run all tests**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass.

**Step 4: Lint and format**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: no warnings.

**Step 5: Full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: `test result: ok`.

**Step 6: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat: wire index_project progress callback into IndexingState"
```

---

## Verification

After all tasks, confirm end-to-end:

1. `cargo build --release` — release binary updated
2. Restart MCP server with `/mcp`
3. Call `index_project({})` on a real project
4. Immediately call `index_status({})` — should show `{"indexing":{"status":"running","done":0,"total":0,"eta_secs":null}}`
5. Poll `index_status` again mid-run — should show `done` counting up with `eta_secs` populated
6. Final `index_status` after completion — `{"indexing":{"status":"done",...}}`
