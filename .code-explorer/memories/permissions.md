# Permissions & Path Security Policy

## Design Intent

The permission model is built around **agent safety without user interruption**. An agent running autonomously should never be able to write outside the current project — this is an intentional hard boundary. If an agent tries to modify a file outside the project root, it gets a `RecoverableError` with a hint, not a fatal crash. The agent can reason about the failure and continue; the user is never interrupted mid-task by a dangerous write slipping through.

## Read Policy: Permissive (deny-list only)

`validate_read_path()` — `src/util/path_security.rs`

- Reads are **allowed anywhere** on the filesystem (absolute or relative)
- Only blocked by a **deny-list** of sensitive locations:
  - `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gcloud`, `~/.docker/config.json`
  - Linux: `/etc/shadow`, `/etc/gshadow`
  - macOS: `/etc/master.passwd`
  - Custom patterns via `[security] denied_read_patterns` in `project.toml`
- Rationale: LLMs legitimately need broad read access (library source, system headers, sibling repos). Reading a credential is bad but far less dangerous than overwriting one.

## Write Policy: Project-root bounded

`validate_write_path()` — `src/util/path_security.rs`

Applies to: `create_file`, `edit_file`, and all symbol-editing tools.

Three sequential checks:

1. **Null/empty rejection** — malformed paths fail immediately
2. **Deny-list first** — sensitive locations blocked before any root check (can't be bypassed by `extra_write_roots`)
3. **Workspace boundary** — canonicalized path must `starts_with(project_root)` or an `extra_write_roots` entry

**Symlink escapes are caught** via parent canonicalization: the parent directory is resolved (not the target file, which may not exist yet), so a symlink pointing outside the root is detected.

Error returned: `RecoverableError` with hint → `isError: false` in MCP → agent sees the problem and a corrective hint, Claude Code does **not** abort sibling parallel calls.

## Escape Hatch: extra_write_roots

Users can explicitly allow writes to additional directories:

```toml
# .code-explorer/project.toml
[security]
extra_write_roots = ["/path/to/other/project"]
```

- Deny-list still applies first (no bypass possible)
- Useful for multi-repo setups where the agent legitimately needs to write across repos

## run_command CWD Restriction

`run_command_inner()` — `src/tools/workflow.rs`

- The `cwd` parameter must be a relative path within the project root
- Canonicalized and checked: `canonical_cwd.starts_with(canonical_root)`, `..` escapes caught
- The **shell command itself** is unrestricted — it can reference any absolute path
- Dangerous commands are separately gated via `acknowledge_risk: true`

## Why writes-only are restricted (not reads)

| | Read | Write |
|---|---|---|
| Boundary | Deny-list only | Project root + deny-list |
| Absolute paths | Always allowed | Only if under allowed root |
| Failure type | RecoverableError | RecoverableError |
| Override | `denied_read_patterns` | `extra_write_roots` |

The asymmetry is intentional. An agent that can read widely but write only to the current project is **both capable and safe** — it can understand any codebase it needs to reference, but cannot accidentally corrupt unrelated projects or system files.

## Key Files

- `src/util/path_security.rs` — `PathSecurityConfig`, `validate_read_path()`, `validate_write_path()`, `canonicalize_write_target()`
- `src/tools/file.rs` — `CreateFile` and `EditFile` call `validate_write_path()` at the top of `call()`
- `src/tools/workflow.rs` — `run_command_inner()` validates `cwd` before spawning the shell
