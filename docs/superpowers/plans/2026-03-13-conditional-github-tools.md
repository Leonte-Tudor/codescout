# Conditional GitHub Tool Registration — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the 4 optional GitHub tools (identity, issue, pr, file) opt-in to save ~2,040 tokens per MCP round-trip; keep github_repo always available.

**Architecture:** Config-gated registration — `from_parts()` reads `security.github_enabled` (default `false`) and conditionally pushes 4 tools + appends a dedicated `github_instructions.md` to the system prompt.

**Tech Stack:** Rust, MCP (rmcp), serde/TOML config

**Spec:** `docs/superpowers/specs/2026-03-13-conditional-github-tools-design.md`

---

## Chunk 1: Config defaults + security gating

### Task 1: Flip config defaults

**Files:**
- Modify: `src/config/project.rs:116-118` (SecuritySection.github_enabled)
- Modify: `src/config/project.rs:137` (SecuritySection Default impl)
- Modify: `src/util/path_security.rs:103` (PathSecurityConfig Default impl)

- [ ] **Step 1: Update SecuritySection field**

In `src/config/project.rs`, change the `github_enabled` field (L116-118) from:

```rust
    /// Enable GitHub tools: github_identity, github_issue, github_pr, github_file, github_repo (default: true)
    #[serde(default = "default_true")]
    pub github_enabled: bool,
```

to:

```rust
    /// Enable additional GitHub tools: github_identity, github_issue, github_pr, github_file.
    /// github_repo is always available. (default: false)
    #[serde(default)]
    pub github_enabled: bool,
```

- [ ] **Step 2: Update SecuritySection Default impl**

In `src/config/project.rs` L137, change:

```rust
            github_enabled: true,
```

to:

```rust
            github_enabled: false,
```

- [ ] **Step 3: Update PathSecurityConfig Default impl**

In `src/util/path_security.rs` L103, change:

```rust
            github_enabled: true,
```

to:

```rust
            github_enabled: false,
```

Also update the doc comment on the field (L82) from:

```rust
    /// Enable GitHub tools (default: true)
```

to:

```rust
    /// Enable additional GitHub tools: identity, issue, pr, file (default: false)
```

- [ ] **Step 4: Update existing config default tests**

In `src/config/project.rs`, find the 3 tests that assert `github_enabled` is true:
- L328: `assert!(sec.github_enabled, "github_enabled should default to true");`
- L338: `assert!(sec.github_enabled);`
- L350: `assert!(sec.github_enabled);`

Change all three to assert `false`:

```rust
assert!(!sec.github_enabled, "github_enabled should default to false");
```

```rust
assert!(!sec.github_enabled);
```

- [ ] **Step 5: Run tests to verify config changes compile and pass**

Run: `cargo test -p codescout -- config`
Expected: All config tests pass with new defaults.

### Task 2: Update security gating

**Files:**
- Modify: `src/util/path_security.rs:345-351` (check_tool_access github arm)
- Modify: `src/util/path_security.rs:1001-1035` (github security tests)

- [ ] **Step 1: Update check_tool_access match arm**

In `src/util/path_security.rs`, change the GitHub match arm (L345-351) from:

```rust
        "github_identity" | "github_issue" | "github_pr" | "github_file" | "github_repo" => {
            if !config.github_enabled {
                bail!(
                    "GitHub tools are disabled. Set security.github_enabled = true in .codescout/project.toml to enable."
                );
            }
        }
```

to:

```rust
        "github_identity" | "github_issue" | "github_pr" | "github_file" => {
            if !config.github_enabled {
                bail!(
                    "GitHub tools (identity/issue/pr/file) are disabled. Set security.github_enabled = true in .codescout/project.toml to enable."
                );
            }
        }
        // github_repo: always allowed, no gate
```

- [ ] **Step 2: Rewrite github security tests**

Replace `github_disabled_blocks_all_github_tools` (L1001-1017) with two tests:

```rust
    #[test]
    fn github_repo_always_allowed() {
        let mut config = PathSecurityConfig::default();
        // default is false, but github_repo should still be allowed
        assert!(
            check_tool_access("github_repo", &config).is_ok(),
            "github_repo should always be allowed regardless of github_enabled"
        );
        config.github_enabled = true;
        assert!(
            check_tool_access("github_repo", &config).is_ok(),
            "github_repo should be allowed when github_enabled is true"
        );
    }

    #[test]
    fn github_disabled_blocks_optional_github_tools() {
        let mut config = PathSecurityConfig::default();
        config.github_enabled = false; // explicit for self-documentation
        for tool in &[
            "github_identity",
            "github_issue",
            "github_pr",
            "github_file",
        ] {
            assert!(
                check_tool_access(tool, &config).is_err(),
                "{} should be blocked when github_enabled is false",
                tool
            );
        }
    }
```

Replace `github_enabled_allows_github_tools` (L1020-1035) with:

```rust
    #[test]
    fn github_enabled_allows_optional_github_tools() {
        let mut config = PathSecurityConfig::default();
        config.github_enabled = true;
        for tool in &[
            "github_identity",
            "github_issue",
            "github_pr",
            "github_file",
        ] {
            assert!(
                check_tool_access(tool, &config).is_ok(),
                "{} should be allowed when github_enabled is true",
                tool
            );
        }
    }
```

Update `check_tool_access_error_message_includes_config_hint` (L980-998) — the test already sets `github_enabled = false` and checks `github_pr`. Since `PathSecurityConfig::default()` now has `github_enabled: false`, simplify:

```rust
    #[test]
    fn check_tool_access_error_message_includes_config_hint() {
        let mut config = PathSecurityConfig::default();
        config.shell_enabled = false;
        let err = check_tool_access("run_command", &config).unwrap_err();
        assert!(
            err.to_string().contains("shell_enabled"),
            "error should mention config key"
        );
        assert!(
            err.to_string().contains("project.toml"),
            "error should mention config file"
        );
        config.github_enabled = false; // explicit for self-documentation
        let err = check_tool_access("github_pr", &config).unwrap_err();
        assert!(
            err.to_string().contains("github_enabled"),
            "error should mention config key"
        );
        // github_repo should NOT be blocked
        assert!(
            check_tool_access("github_repo", &config).is_ok(),
            "github_repo should not be gated"
        );
    }
```

- [ ] **Step 3: Run security tests**

Run: `cargo test -p codescout -- path_security`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/config/project.rs src/util/path_security.rs
git commit -m "feat(config): default github_enabled to false, ungating github_repo"
```

---

## Chunk 2: Server instructions split + prompt builder

### Task 3: Create github_instructions.md

**Files:**
- Create: `src/prompts/github_instructions.md`

- [ ] **Step 1: Create the file**

Create `src/prompts/github_instructions.md` with the content that was previously in `server_instructions.md` for the 4 optional tools:

```markdown
## GitHub Tools

These tools are enabled via `security.github_enabled = true` in `.codescout/project.toml`.

### When to use

| Task | Tool | NOT this |
|---|---|---|
| GitHub identity / teams | `github_identity(method, ...)` | — |
| GitHub issues | `github_issue(method, owner, repo, ...)` | — |
| GitHub pull requests | `github_pr(method, owner, repo, ...)` | — |
| GitHub file contents / writes | `github_file(method, owner, repo, path, ...)` | — |

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("gh issue list")` or `run_command("gh pr ...")` | `github_issue(method, owner, repo, ...)` / `github_pr(...)` | Structured output, pagination, buffer handling built-in |

### Reference

- `github_identity(method)` — authenticated user profile, team membership, user search.
  - `method`: `get_me` | `search_users` (query required) | `get_teams` | `get_team_members` (org + team_slug required)
- `github_issue(method, owner, repo, ...)` — issue read/write operations.
  - Read: `list` | `search` | `get` | `get_comments` | `get_labels` | `get_sub_issues`
  - Write: `create` (title required) | `update` | `add_comment` | `add_sub_issue` | `remove_sub_issue`
  - `limit` defaults to 30 for list/search.
- `github_pr(method, owner, repo, ...)` — pull request read/write operations.
  - Read: `list` | `search` | `get` | `get_diff` | `get_files` | `get_comments` | `get_reviews` | `get_review_comments` | `get_status`
  - Write: `create` | `update` | `merge` | `update_branch` | `create_review` | `submit_review` | `delete_review` | `add_review_comment` | `add_reply_to_comment`
  - `get_diff` always returns a `@tool` buffer handle (diffs are large).
- `github_file(method, owner, repo, path, ...)` — file contents and writes via GitHub API.
  - `get` — fetch file at optional ref/branch (returns `@buffer` handle).
  - `create_or_update` — create or update a single file (`sha` required when updating).
  - `delete` — delete a file (`sha` required).
  - `push_files` — push multiple files in a single commit.
```

### Task 4: Update server_instructions.md

**Files:**
- Modify: `src/prompts/server_instructions.md:36,61-66,83,177-201`

- [ ] **Step 1: Update "By knowledge level" table row**

In `src/prompts/server_instructions.md` L36, change:

```markdown
| **A GitHub repo/issue/PR** | `github_repo` / `github_issue` / `github_pr` | drill with specific `method` parameter |
```

to:

```markdown
| **A GitHub repo** | `github_repo` | drill with specific `method` parameter |
```

- [ ] **Step 2: Update "By task" table**

Remove these 4 rows (L62-65):

```markdown
| GitHub identity / teams | `github_identity(method, ...)` | — |
| GitHub issues | `github_issue(method, owner, repo, ...)` | — |
| GitHub pull requests | `github_pr(method, owner, repo, ...)` | — |
| GitHub file contents / writes | `github_file(method, owner, repo, path, ...)` | — |
```

Keep the `github_repo` row (L66):

```markdown
| GitHub repo / branches / releases | `github_repo(method, ...)` | — |
```

- [ ] **Step 3: Update anti-patterns table**

Remove this row (L83):

```markdown
| `run_command("gh issue list")` or `run_command("gh pr ...")` | `github_issue(method, owner, repo, ...)` / `github_pr(...)` | Structured output, pagination, buffer handling built-in |
```

Keep the `github_repo(list_commits)` row (L84).

- [ ] **Step 4: Replace `### GitHub` section**

Replace the entire `### GitHub` section (L177-201) with:

```markdown
### GitHub

`github_repo` is for operations that require the GitHub API — code search across repos,
listing releases/tags, creating branches remotely, forking. For local history (blame, log,
diff), prefer `run_command("git ...")` — it's faster and has full history.

- `github_repo(method, ...)` — repository, branch, commit, release, and code search.
  - Repo: `search` | `create` | `fork`
  - Branches: `list_branches` | `create_branch`
  - Commits: `list_commits` | `get_commit` (returns `@buffer` handle)
  - Releases: `list_releases` | `get_latest_release` | `get_release_by_tag`
  - Tags: `list_tags` | `get_tag`
  - Code: `search_code` (returns `@buffer` handle)

Additional GitHub tools (`github_identity`, `github_issue`, `github_pr`, `github_file`)
are available when `security.github_enabled = true` in `.codescout/project.toml`.
Restart the server after changing this setting.
```

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/github_instructions.md
git commit -m "feat(prompts): split GitHub instructions, github_repo always documented"
```

### Task 5: Update prompt builder + ProjectStatus

**Files:**
- Modify: `src/prompts/mod.rs:8,58-65,12-54,161-176` (constant, struct, builder, tests)
- Modify: `src/agent.rs:202-224` (project_status populates github_enabled)

- [ ] **Step 1: Add GITHUB_INSTRUCTIONS constant**

In `src/prompts/mod.rs`, after the `SERVER_INSTRUCTIONS` constant (L8), add:

```rust
pub const GITHUB_INSTRUCTIONS: &str = include_str!("github_instructions.md");
```

- [ ] **Step 2: Add github_enabled field to ProjectStatus**

In `src/prompts/mod.rs`, add a field to `ProjectStatus` (L58-65):

```rust
pub struct ProjectStatus {
    pub name: String,
    pub path: String,
    pub languages: Vec<String>,
    pub memories: Vec<String>,
    pub has_index: bool,
    pub system_prompt: Option<String>,
    pub github_enabled: bool,
}
```

- [ ] **Step 3: Update build_server_instructions to conditionally append GitHub instructions**

In `src/prompts/mod.rs`, inside `build_server_instructions` (L12-54), add **before** the `system_prompt` block (so custom instructions come last and can override):

```rust
        if status.github_enabled {
            instructions.push_str("\n\n");
            instructions.push_str(GITHUB_INSTRUCTIONS);
        }
```

This goes right before `if let Some(prompt) = &status.system_prompt {`.

- [ ] **Step 4: Populate github_enabled in Agent::project_status**

In `src/agent.rs`, update the `project_status` method (L202-224). Add after `let has_index` (L206):

```rust
        let github_enabled = project.config.security.github_enabled;
```

And add the field to the returned struct (after `system_prompt`):

```rust
            github_enabled,
```

- [ ] **Step 5: Update all existing test ProjectStatus literals**

In `src/prompts/mod.rs`, every test that constructs a `ProjectStatus` needs `github_enabled: false` added. There are 5 tests (L161, L179, L256, L275 and any others constructing ProjectStatus). Add `github_enabled: false,` after `system_prompt` in each.

For example, `build_with_project_appends_status` (L161-176) becomes:

```rust
    fn build_with_project_appends_status() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/home/user/my-project".into(),
            languages: vec!["rust".into(), "python".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            github_enabled: false,
        };
        // ... rest unchanged
    }
```

- [ ] **Step 6: Add test for GitHub instructions inclusion**

In `src/prompts/mod.rs`, add a new test:

```rust
    #[test]
    fn build_with_github_enabled_appends_github_instructions() {
        let status = ProjectStatus {
            name: "test".into(),
            path: "/tmp/test".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            github_enabled: true,
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("github_identity"),
            "should include GitHub tool docs when enabled"
        );
        assert!(
            result.contains("github_pr"),
            "should include GitHub PR docs when enabled"
        );
    }

    #[test]
    fn build_without_github_excludes_github_instructions() {
        let status = ProjectStatus {
            name: "test".into(),
            path: "/tmp/test".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            github_enabled: false,
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            !result.contains("github_identity"),
            "should NOT include optional GitHub tool docs when disabled"
        );
    }
```

- [ ] **Step 7: Run prompt tests**

Run: `cargo test -p codescout -- prompts`
Expected: All pass including new tests.

- [ ] **Step 8: Commit**

```bash
git add src/prompts/mod.rs src/agent.rs
git commit -m "feat(prompts): conditionally append GitHub instructions based on config"
```

---

## Chunk 3: Conditional tool registration + server tests

### Task 6: Conditional tool registration in from_parts

**Files:**
- Modify: `src/server.rs:57-107` (from_parts)

- [ ] **Step 1: Move GitHub tools to conditional block**

In `src/server.rs`, modify `from_parts` (L57-107). Move `GithubRepo` to the end of the always-registered vec, and push the other 4 conditionally. The `vec![...]` ending changes from:

```rust
            // GitHub tools
            Arc::new(github::GithubIdentity),
            Arc::new(github::GithubIssue),
            Arc::new(github::GithubPr),
            Arc::new(github::GithubFile),
            Arc::new(github::GithubRepo),
        ];
```

to:

```rust
            // GitHub tools — github_repo always available
            Arc::new(github::GithubRepo),
        ];

        // Optional GitHub tools (identity/issue/pr/file) — opt-in via config
        let github_enabled = agent.security_config().await.github_enabled;
        if github_enabled {
            tools.push(Arc::new(github::GithubIdentity));
            tools.push(Arc::new(github::GithubIssue));
            tools.push(Arc::new(github::GithubPr));
            tools.push(Arc::new(github::GithubFile));
        }
```

- [ ] **Step 2: Run build to verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

### Task 7: Update server tests

**Files:**
- Modify: `src/server.rs:516-578` (make_server, server_registers_all_tools)

- [ ] **Step 1: Update server_registers_all_tools for 25 tools (default)**

Change the test (L530-578) to expect 25 tools (no optional GitHub tools):

```rust
    #[tokio::test]
    async fn server_registers_all_tools() {
        let (_dir, server) = make_server().await;
        let expected_tools = [
            "read_file",
            "list_dir",
            "search_pattern",
            "create_file",
            "find_file",
            "edit_file",
            "run_command",
            "onboarding",
            "find_symbol",
            "find_references",
            "list_symbols",
            "replace_symbol",
            "insert_code",
            "rename_symbol",
            "remove_symbol",
            "goto_definition",
            "hover",
            "memory",
            "semantic_search",
            "index_project",
            "index_status",
            "activate_project",
            "project_status",
            "list_libraries",
            "github_repo",
        ];
        assert_eq!(
            server.tools.len(),
            expected_tools.len(),
            "tool count mismatch: expected {}, got {}\nregistered: {:?}",
            expected_tools.len(),
            server.tools.len(),
            server.tools.iter().map(|t| t.name()).collect::<Vec<_>>()
        );
        for name in &expected_tools {
            assert!(
                server.find_tool(name).is_some(),
                "tool '{}' not found in server",
                name
            );
        }
    }
```

- [ ] **Step 2: Add test for github_enabled=true registration**

Add a helper and new test:

```rust
    async fn make_server_with_github() -> (tempfile::TempDir, CodeScoutServer) {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("project.toml"),
            "[security]\ngithub_enabled = true\n",
        )
        .unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let server = CodeScoutServer::new(agent).await;
        (dir, server)
    }

    #[tokio::test]
    async fn server_registers_github_tools_when_enabled() {
        let (_dir, server) = make_server_with_github().await;
        assert_eq!(
            server.tools.len(),
            29,
            "should have 29 tools with github_enabled=true, got {}\nregistered: {:?}",
            server.tools.len(),
            server.tools.iter().map(|t| t.name()).collect::<Vec<_>>()
        );
        for name in &["github_identity", "github_issue", "github_pr", "github_file"] {
            assert!(
                server.find_tool(name).is_some(),
                "tool '{}' should be registered when github_enabled=true",
                name
            );
        }
    }
```

- [ ] **Step 3: Run all server tests**

Run: `cargo test -p codescout -- server`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "feat(server): conditionally register GitHub tools based on github_enabled"
```

---

## Chunk 4: Full test suite + final verification

### Task 8: Run full test suite and verify

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: All tests pass (including agent, prompts, server, path_security tests).

- [ ] **Step 4: Build release**

Run: `cargo build --release`
Expected: Compiles successfully.

- [ ] **Step 5: Final commit (if any fixups needed)**

If fmt/clippy/tests required changes:
```bash
git add -u
git commit -m "style: fmt + clippy fixups for conditional GitHub tools"
```
