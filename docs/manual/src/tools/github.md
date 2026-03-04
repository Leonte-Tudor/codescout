# GitHub Tools

codescout includes five tools for authenticated GitHub access — reading and writing issues,
pull requests, files, and repository metadata. These tools require a GitHub token configured
in your MCP host environment (`GITHUB_TOKEN` or equivalent).

---

## `github_identity`

**Purpose:** Identity and team operations — get the authenticated user, search GitHub users,
or inspect team membership.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | yes | `get_me` \| `search_users` \| `get_teams` \| `get_team_members` |
| `query` | string | for `search_users` | Search query |
| `org` | string | for `get_team_members` | Organization login |
| `team_slug` | string | for `get_team_members` | Team slug |

**Methods:**
- `get_me` — returns the authenticated user's profile (login, name, email, bio)
- `search_users` — search GitHub users by query string
- `get_teams` — list all teams the authenticated user belongs to
- `get_team_members` — list members of a specific team (requires `org` + `team_slug`)

**Example:**

```json
{
  "tool": "github_identity",
  "arguments": { "method": "get_me" }
}
```

---

## `github_issue`

**Purpose:** Read and write GitHub issues and comments.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | yes | See methods below |
| `owner` | string | most methods | Repository owner (user or org) |
| `repo` | string | most methods | Repository name |
| `number` | integer | single-issue methods | Issue number |
| `title` | string | `create` | Issue title |
| `body` | string | `create`/`update`/`add_comment` | Issue or comment body |
| `state` | string | `list`/`update` | `"open"` or `"closed"` |
| `labels` | string | `list`/`create`/`update` | Comma-separated label names |
| `assignees` | string | `create`/`update` | Comma-separated login names |
| `query` | string | `search` | Search query |
| `limit` | integer | `list`/`search` | Max results (default 30) |

**Read methods:** `list` \| `search` \| `get` \| `get_comments` \| `get_labels` \| `get_sub_issues`

**Write methods:** `create` \| `update` \| `add_comment` \| `add_sub_issue` \| `remove_sub_issue`

**Example — create an issue:**

```json
{
  "tool": "github_issue",
  "arguments": {
    "method": "create",
    "owner": "acme",
    "repo": "myapp",
    "title": "Fix null pointer in auth middleware",
    "body": "Reproduces when the token is missing the `sub` claim."
  }
}
```

**Example — list open issues:**

```json
{
  "tool": "github_issue",
  "arguments": {
    "method": "list",
    "owner": "acme",
    "repo": "myapp",
    "state": "open",
    "limit": 10
  }
}
```

---

## `github_pr`

**Purpose:** Read and write pull requests — including diffs, reviews, and merges.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | yes | See methods below |
| `owner` | string | most methods | Repository owner |
| `repo` | string | most methods | Repository name |
| `number` | integer | single-PR methods | PR number |
| `title` | string | `create`/`update` | PR title |
| `body` | string | `create`/`update`/review | PR or review body |
| `base` | string | `create`/`update` | Base branch |
| `head` | string | `create` | Head branch (`user:branch`) |
| `state` | string | `list`/`update` | `"open"` or `"closed"` |
| `draft` | boolean | `create`/`update` | Draft status |
| `merge_method` | string | `merge` | `"merge"` \| `"squash"` \| `"rebase"` |
| `event` | string | `create_review` | `"APPROVE"` \| `"REQUEST_CHANGES"` \| `"COMMENT"` |
| `query` | string | `search` | Search query |
| `limit` | integer | `list`/`search` | Max results (default 30) |

**Read methods:** `list` \| `search` \| `get` \| `get_diff` \| `get_files` \| `get_comments` \| `get_reviews` \| `get_review_comments` \| `get_status`

**Write methods:** `create` \| `update` \| `merge` \| `update_branch` \| `create_review` \| `submit_review` \| `delete_review` \| `add_review_comment` \| `add_reply_to_comment`

**Important:** `get_diff` always returns a `@tool_*` buffer handle — diffs can be very large.
Query the buffer: `run_command("grep '+' @tool_abc123")`.

**Example — get a PR diff:**

```json
{
  "tool": "github_pr",
  "arguments": {
    "method": "get_diff",
    "owner": "acme",
    "repo": "myapp",
    "number": 42
  }
}
```

Then query: `run_command("grep '^+' @tool_xxxx")`

**Example — approve a PR:**

```json
{
  "tool": "github_pr",
  "arguments": {
    "method": "create_review",
    "owner": "acme",
    "repo": "myapp",
    "number": 42,
    "event": "APPROVE",
    "body": "Looks good — tests pass and the logic matches the spec."
  }
}
```

---

## `github_file`

**Purpose:** Read and write files in a GitHub repository via the GitHub API. Use for
pushing changes without a local clone, or reading files at a specific ref.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | yes | `get` \| `create_or_update` \| `delete` \| `push_files` |
| `owner` | string | yes | Repository owner |
| `repo` | string | yes | Repository name |
| `path` | string | most methods | File path within the repository |
| `ref` | string | `get` | Branch, tag, or commit SHA |
| `content` | string | `create_or_update` | Base64-encoded file content |
| `message` | string | write methods | Commit message |
| `branch` | string | write methods | Target branch |
| `sha` | string | `create_or_update`/`delete` | Blob SHA of existing file (required when updating) |
| `files` | array | `push_files` | `[{path, content}]` array for multi-file commits |

**Methods:**
- `get` — fetch file contents at an optional ref (returns `@buffer` handle for large files)
- `create_or_update` — create or update a single file; `sha` required when updating
- `delete` — delete a file; `sha` required
- `push_files` — push multiple files in a single commit

**Example — push multiple files:**

```json
{
  "tool": "github_file",
  "arguments": {
    "method": "push_files",
    "owner": "acme",
    "repo": "myapp",
    "branch": "main",
    "message": "Add config and readme",
    "files": [
      { "path": "config/default.toml", "content": "[server]\nport = 8080\n" },
      { "path": "docs/setup.md", "content": "# Setup\n\nRun `cargo run`.\n" }
    ]
  }
}
```

**Tips:**
- `sha` is returned by `get` — always fetch it before updating or deleting a file.
- `push_files` is the most efficient way to push multiple file changes in one commit.

---

## `github_repo`

**Purpose:** Repository, branch, commit, release, tag, and code search operations.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `method` | string | yes | See methods below |
| `owner` | string | most methods | Repository owner |
| `repo` | string | most methods | Repository name |
| `query` | string | `search`/`search_code` | Search query |
| `name` | string | `create` | New repository name |
| `private` | boolean | `create` | Private repository flag |
| `branch` | string | `create_branch` | New branch name |
| `from_branch` | string | `create_branch` | Source branch (default: HEAD) |
| `sha` | string | `get_commit` | Commit SHA |
| `tag` | string | release/tag methods | Tag name |
| `limit` | integer | list methods | Max results (default 30) |

**Repo methods:** `search` \| `create` \| `fork`

**Branch methods:** `list_branches` \| `create_branch`

**Commit methods:** `list_commits` \| `get_commit` (returns `@buffer` handle)

**Release methods:** `list_releases` \| `get_latest_release` \| `get_release_by_tag`

**Tag methods:** `list_tags` \| `get_tag`

**Code:** `search_code` (returns `@buffer` handle)

**Example — search code:**

```json
{
  "tool": "github_repo",
  "arguments": {
    "method": "search_code",
    "query": "authenticate_user repo:acme/myapp language:rust"
  }
}
```

Then query: `run_command("grep 'path' @tool_xxxx")`

**Example — create a branch:**

```json
{
  "tool": "github_repo",
  "arguments": {
    "method": "create_branch",
    "owner": "acme",
    "repo": "myapp",
    "branch": "feat/new-auth",
    "from_branch": "main"
  }
}
```

**Tips:**
- `get_commit` and `search_code` return buffer handles — query them with `run_command`.
- For code search, GitHub's query syntax supports `repo:`, `language:`, `path:`, `extension:` filters.
