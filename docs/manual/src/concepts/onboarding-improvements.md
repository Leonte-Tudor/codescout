# Onboarding Improvements

Two improvements to the `onboarding` tool that make it safer for large
projects and more resilient to tool API changes.

---

## Subagent Delegation

Onboarding now offloads project exploration to a **dedicated subagent**
instead of performing it inline. This prevents the exploration phase —
which can involve dozens of tool calls across a large codebase — from
exhausting the main agent's context window.

### How it works

When `onboarding` is called on a project that hasn't been onboarded yet,
it returns a two-part response:

1. **`main_agent_instructions`** (~200 tokens) — short instructions telling
   the calling agent to dispatch a Sonnet subagent.
2. **`subagent_prompt`** — a self-contained prompt the calling agent passes
   verbatim to the subagent. Contains: preamble, step-by-step exploration
   instructions, memory templates, and an epilogue.

The subagent performs all exploration (file reads, symbol scans, language
detection) and writes the project memories. The main agent's context stays
clean.

```
Agent calls onboarding()
  → receives dispatch instructions + subagent_prompt
  → spawns subagent with subagent_prompt
     → subagent explores codebase
     → subagent writes memory files
  → main agent continues with fresh context
```

### Fast path unchanged

If the project is already onboarded, `onboarding()` returns a short status
message as before — no subagent is involved.

---

## Version-Aware System Prompt Refresh

The `onboarding` tool now tracks a version number (`ONBOARDING_VERSION`)
stored in `.codescout/project.toml`. When a project's stored version is
older than the current server's version, onboarding **automatically
dispatches a lightweight refresh subagent** to regenerate the system prompt
from existing memories — without re-exploring the codebase.

### Why it exists

When codescout's tool API changes (renames, new tools, removed parameters),
existing projects carry a system prompt that references old tool names. The
refresh detects the version mismatch and rebuilds the prompt from current
templates, so the agent's guidance stays accurate without requiring a full
re-onboard.

### Behavior

| Stored version | Action |
|---|---|
| Missing (pre-versioning project) | Triggers refresh |
| Lower than `ONBOARDING_VERSION` | Triggers refresh |
| Equal to `ONBOARDING_VERSION` | No-op (already current) |
| Higher (downgrade scenario) | No-op (avoids churn) |

### refresh_prompt parameter

To force a prompt refresh explicitly — for example, after updating memories
manually — pass `refresh_prompt=true`:

```
onboarding(refresh_prompt=true)
```

This regenerates the system prompt from current memories and templates
without re-scanning the project. Useful after bulk memory edits or after
upgrading codescout to a new version that bumps `ONBOARDING_VERSION`.

### What gets refreshed

The refresh subagent reads existing project memories and rewrites the system
prompt section of `.codescout/project.toml`. It does not re-read source
files or re-scan the project structure — only the prompt template is
regenerated.
