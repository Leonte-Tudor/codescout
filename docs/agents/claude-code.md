# Claude Code

## One-Time Setup

Prerequisites: Rust toolchain, `cargo install codescout`. The binary lands at `~/.cargo/bin/codescout`.

Register codescout as an MCP server. The recommended approach is user-level registration — edit `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start"],
      "type": "stdio"
    }
  }
}
```

For a project-scoped alternative, place a `.mcp.json` file at the project root with the same block.

## Workflow Skills

Claude Code handles workflow skills differently from Copilot/Cursor — skills are loaded via the Superpowers plugin system, not manually installed files. No manual skill file installation is needed; skills activate automatically once the companion plugin is set up. See [Superpowers workflow](../manual/src/concepts/superpowers.md) for details.

## Routing Plugin (codescout-companion)

The routing plugin (codescout-companion) is a Claude Code plugin that enforces codescout tool use. It adds a `PreToolUse` hook that blocks `Read`, `Grep`, and `Glob` on source files and redirects to the appropriate codescout tool.

Install via:

```
claude plugin install codescout-companion
```

Or follow the [Routing Plugin guide](../manual/src/getting-started/routing-plugin.md) for manual setup via `~/.claude/settings.json`.

## Verify

Restart Claude Code, then run `/mcp` — confirm `codescout` appears as connected. Then ask: "What symbols are in src/main.rs?" — Claude should call `mcp__codescout__list_symbols`, not read the file.

## Day-to-Day Workflow

codescout injects tool guidance automatically into every session via the MCP system prompt. For the full disciplined development workflow, see:

- [Superpowers workflow](../manual/src/concepts/superpowers.md)
- [Tool Reference](../manual/src/tools/overview.md)
- [Progressive Disclosure](../manual/src/concepts/progressive-disclosure.md)
