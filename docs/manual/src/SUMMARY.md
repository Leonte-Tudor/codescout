# Summary

[Introduction](introduction.md)
[From code-explorer to codescout](history.md)

# User Guide

- [Why codescout?](why-codescout.md)

- [Installation](getting-started/installation.md)
  - [Your First Project](getting-started/first-project.md)
  - [Routing Plugin](getting-started/routing-plugin.md)

- [Agent Integrations](agents/overview.md)
  - [Claude Code](agents/claude-code.md)
  - [GitHub Copilot](agents/copilot.md)
  - [Cursor](agents/cursor.md)

- [Progressive Disclosure](concepts/progressive-disclosure.md)
  - [Output Modes](concepts/output-modes.md)
  - [Tool Selection](concepts/tool-selection.md)

- [Shell Integration](concepts/shell-integration.md)
  - [Output Buffers](concepts/output-buffers.md)
  - [Interactive Sessions](concepts/elicitation-interactive-sessions.md)

- [Semantic Search](concepts/semantic-search.md)
  - [Setup Guide](semantic-search-guide.md)

- [Library Navigation](concepts/library-navigation.md)
  - [Auto-Registration](concepts/multi-ecosystem-auto-registration.md)
- [Multi-Project Workspaces](concepts/multi-project-workspace.md)
  - [activate_project Output](concepts/activate-project-output-optimization.md)
- [LSP Idle TTL](concepts/lsp-idle-ttl.md)

- [Memory](concepts/memory.md)
  - [After Onboarding](concepts/after-onboarding.md)
  - [Sections Filter](concepts/memory-sections-filter.md)

- [Dashboard](concepts/dashboard.md)
  - [LSP Startup Statistics](concepts/lsp-startup-stats.md)
- [Git Worktrees](concepts/worktrees.md)

- [Security & Permissions](concepts/security.md)
  - [Security Profiles](concepts/security-profiles.md)
  - [Compact Schemas & `activate_project` Safety](concepts/compact-schemas-and-activate-project-safety.md)
  - [PostCompact Cache Flush](concepts/post-compact-cache-flush.md)

- [Routing Plugin](concepts/routing-plugin.md)
  - [Superpowers Workflow](concepts/superpowers.md)

- [Project Configuration](configuration/project-toml.md)
  - [Embedding Backends](configuration/embedding-backends.md)

- [Language Support](language-support.md)

# Tool Reference

- [Tools Overview](tools/overview.md)
  - [Symbol Navigation](tools/symbol-navigation.md)
  - [File Operations](tools/file-operations.md)
  - [Editing](tools/editing.md)
    - [Structural Edit Gate](tools/edit-file-structural-gate.md)
  - [Semantic Search](tools/semantic-search.md)
  - [Library Navigation](tools/library-navigation.md)
  - [Git](tools/git.md)
  - [AST Analysis](tools/ast.md)
  - [Memory](tools/memory.md)
  - [Workflow & Config](tools/workflow-and-config.md)
    - [Read-Only `activate_project`](tools/activate-project-read-only.md)
  - [GitHub](tools/github.md)

# Development

- [Architecture](architecture.md)
- [Adding Languages](extending/adding-languages.md)
  - [Writing Tools](extending/writing-tools.md)
  - [The Tool Trait](extending/tool-trait.md)

- [Diagnostic Logging](concepts/diagnostic-logging.md)
- [Troubleshooting](troubleshooting.md)
