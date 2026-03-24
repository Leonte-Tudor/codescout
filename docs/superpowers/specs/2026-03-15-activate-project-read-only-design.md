# activate_project Read-Only Default

**Date:** 2026-03-15
**Status:** Approved

## Problem

When an LLM activates a project different from the one it started with (e.g., to browse
code in another repo), all write tools are available by default. This is dangerous:
the LLM might accidentally edit files in a project it's only visiting for reference.

The `file_write_enabled` gate and `check_tool_access` already enforce write restrictions,
but they're wired to a static config setting — not to the activation context.

## Design

### Behavior

- When `activate_project` is called with a path **different from `home_root`**, the
  project activates with `read_only: true` by default.
- The response hint includes: `"This project is activated in read-only mode. To enable
  writes, call activate_project with read_only: false."`
- When called with `read_only: false` explicitly, writes are enabled regardless of
  whether it matches the home project.
- Returning to `home_root` (re-activating the initial project) always restores writable
  mode — no need to pass `read_only: false`.
- `switch_focus` (by project ID within workspace) inherits the write setting from the
  parent project — unchanged.

### Implementation (3 touch points)

#### 1. `ActiveProject` — `src/agent.rs`

Add a `read_only: bool` field:

```rust
pub struct ActiveProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub memory: MemoryStore,
    pub private_memory: MemoryStore,
    pub library_registry: LibraryRegistry,
    pub dirty_files: Arc<Mutex<HashSet<PathBuf>>>,
    pub read_only: bool,  // NEW
}
```

Default to `false` in all existing construction sites.

#### 2. `Agent::activate()` — `src/agent.rs`

Accept an optional `read_only: Option<bool>` parameter. Logic:

```
if read_only == Some(false):
    project.read_only = false          // explicit opt-in to writes
else if path == home_root:
    project.read_only = false          // home project always writable
else:
    project.read_only = true           // non-home defaults to read-only
```

#### 3. `ActivateProject` tool — `src/tools/config.rs`

- Add optional `read_only` boolean to the input schema.
- Pass it through to `Agent::activate()`.
- Include `read_only: true/false` in the response JSON.
- When read-only, append to the hint: `"This project is activated in read-only mode.
  To enable writes, call activate_project with read_only: false."`

### Security Config Wiring

`Agent::security_config()` builds `PathSecurityConfig` from `ActiveProject`. Add:

```rust
if project.read_only {
    config.file_write_enabled = false;
}
```

This automatically gates all write tools (`create_file`, `edit_file`, `replace_symbol`,
`insert_code`, `rename_symbol`, `remove_symbol`) via the existing `check_tool_access`
function in `server.rs`. No changes needed to `check_tool_access` or any write tool.

### What stays unchanged

- `switch_focus` — no changes
- `validate_write_path` — unchanged (the gate is upstream in `check_tool_access`)
- `check_tool_access` in `server.rs` — unchanged (already reads `file_write_enabled`)
- Server instructions / prompts — the hint in the response is self-documenting
- `run_command` — stays enabled (read-only means no file writes, not no shell)

## Testing

1. **Unit test:** `activate_project` to a non-home path → verify `read_only == true`
2. **Unit test:** `activate_project` to a non-home path with `read_only: false` → verify writable
3. **Unit test:** `activate_project` back to `home_root` → verify `read_only == false`
4. **Integration test:** attempt `edit_file` on a read-only project → verify `RecoverableError`
5. **Response test:** verify hint text includes read-only notice when applicable
