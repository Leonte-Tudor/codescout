# MCP Elicitation Integration — Work Items

> Temporary tracking file. Remove after all items are implemented.
> Research: [`docs/research/2026-03-15-mcp-elicitation-claude-code-updates.md`](research/2026-03-15-mcp-elicitation-claude-code-updates.md)

## Prerequisites

- [x] **E-0: Elicitation plumbing** — Add `elicitation/requestInput` support to
  `ToolContext`. Requires rmcp client capability check + a helper like
  `ctx.elicit(message, schema) -> ElicitResult`. Use `schemars` to generate
  the JSON Schema subset (string, boolean, number, enum). Wire through
  `ServerHandler` so any tool can call it.

## Work Items

- [-] **E-1: Tool disambiguation (`find_symbol`)** — Removed. Elicitation is
  server→human, not server→AI. Disambiguation should be handled autonomously by
  the AI agent based on context and heuristics (LLM can reason about which match
  is most likely given the conversation state).

- [-] **E-2: Dangerous command confirmation (`run_command`)** — Removed. The
  two-round-trip `pending_ack` / `acknowledge_risk` pattern works well for
  autonomous AI agents. Elicitation disrupts the agent's autonomy and should be
  reserved for interactive human input, not confirmation from an LLM.

- [x] **E-3: Interactive sessions via elicitation** — Spike implemented on
  `experiments` branch (commit: `spike(elicitation): E-3 interactive sessions
  via elicitation loop`). See `docs/manual/src/experimental/elicitation-interactive-sessions.md`.

  **What was implemented:**
  - `run_command(interactive: true)` added as a new branch in `RunCommand::call`.
  - `run_command_interactive()` in `src/tools/workflow.rs`: spawns the process
    with piped stdin/stdout/stderr, drives it via `ctx.elicit()` in a loop.
  - Settle detection: 150 ms silence window between reads (separate stdout/stderr
    buffers to avoid Rust double-borrow). Max 50 rounds guard against runaway loops.
  - Elicitation prompt shows last 4000 chars of accumulated output per round.
  - Empty input = cancel + kill process. Natural exit detected via `child.try_wait()`.
  - No dangerous-command elicitation in interactive mode (blocked outright) to keep
    spike scope tight.

  **UX assessment:**
  - Practical for slow-interaction CLIs: setup wizards, `cargo new`-style prompts,
    Python/Node REPLs with human think-time between inputs.
  - Unsuitable for high-frequency interactive programs (e.g. vim, ncurses TUIs):
    each round-trip adds ~1–3 s MCP latency, and the elicitation dialog is a form
    field, not a terminal.
  - The settle window (150 ms) is a heuristic — programs that emit output in bursts
    with longer pauses may split a logical prompt across two elicitation rounds.

  **Blockers / concerns:**
  - The inner `async fn drain_with_settle` pattern (nested async fn) compiles fine
    but is an unusual Rust pattern. An alternative would be a module-level helper.
  - `select!` on two timeout-wrapped reads breaks cleanly on the first EOF/timeout,
    but does not drain the *other* reader — a process that writes only to stderr
    after the stdout branch times out will have that stderr missed until the next
    round. Low-priority for the spike but worth fixing before graduation.
  - No test coverage: interactive mode needs a live MCP peer. Consider a mock-peer
    integration test in a follow-up.

- [-] **E-4: Mutation confirmation (`replace_symbol`, `remove_symbol`)** —
  Removed. Same reasoning as E-1 and E-2: disambiguation and confirmation should
  be handled autonomously by the AI agent, not via server-to-human elicitation.

- [x] **E-5: `PostCompact` hook integration** — Register for the Claude Code
  `PostCompact` hook. On fire: invalidate stale LSP position caches (symbol
  positions may have shifted if the user edited files during compaction window),
  and optionally re-inject a fresh project status summary into the next
  server instructions.

- [x] **E-6: Smart interceptor proposals (companion plugin)** — Upgrade the
  `code-explorer-routing` PreToolUse hooks from hard blocks to elicitation-driven
  proposals. Instead of "WRONG TOOL. You called Grep on source files", the hook
  proposes a fix via elicitation:
  - `Grep` on source → "Search with `search_pattern` instead? (or `find_symbol` if
    you're looking for a symbol name)"
  - `Grep` on cargo registry path → "Register as library and use `find_symbol(scope=...)`?"
  - `Read` on source → "Use `list_symbols` + `find_symbol(include_body=true)` instead?"
  - `Bash` with shell command → "Use `run_command` instead?"
  The hook script would use the `Elicitation` hook to auto-respond with the
  corrected tool call, or present the choice to the user. See BUG-022 for the
  library discovery gap this addresses.

  **Subparts:**
  - [ ] E-6a: Research whether PreToolUse hooks can trigger elicitation (they may
    only return allow/block — elicitation might need to happen inside the MCP
    server after the block, not in the hook itself)
  - [ ] E-6b: If hooks can't elicit, add a `suggest_alternative` field to
    `RecoverableError` that the server returns when a tool is used suboptimally
    (not blocked, but guided)
  - [ ] E-6c: Auto-register top-N dependencies as libraries during
    `activate_project` so `find_symbol(scope="lib:...")` is always available
    without manual `register_library` (fixes BUG-022 root cause)
