# Documentation Restructure Design

> Research compiled 2026-03-13.

---

## Goal

Make codescout's documentation beginner-friendly without losing depth. The README becomes a billboard (~80 lines), agent integrations become first-class standalone guides, and multi-agent research is published in sanitized form.

## Architecture

Four parallel tracks:

1. **README** — stripped to TLDR + comparison + quick start + agent integrations + research link
2. **`docs/agents/`** — new standalone setup guides per agent (Claude Code, Copilot, Cursor), adapted from existing internal files, sanitized for public use
3. **`docs/research/`** — sanitized copy of multi-agent context loss research
4. **`docs/manual/`** — two new chapters: "Agent Integrations" and "Why codescout?"

Single source of truth: `docs/agents/` files are referenced from the manual (no duplication).

---

## Section 1: README (~80 lines)

### Structure

```
# codescout
<tagline>

## What it does
3 bullets: symbol navigation, semantic search, token efficiency

## Why not just read files?
Before/after comparison table

## Quick start
cargo install + 2-line MCP config + link to full guide

## Agent integrations
Claude Code | GitHub Copilot | Cursor — one line each + links to docs/agents/

## Multi-agent infrastructure
2 sentences + link to docs/research/multi-agent-context-loss.md

## Tools (29)
One-liner: category counts · language list · link to tool reference

## Contributing / License
```

### Comparison table

| Without codescout | With codescout |
|---|---|
| Agent reads full files to find one function | Navigates by symbol name — zero file reads |
| `grep` returns noise (comments, strings, docs) | `find_references` returns exact call sites |
| Context burns on navigation overhead | Token-efficient by design — compact by default |
| State lost between sessions | Persistent memory across sessions |

### Research callout (2 sentences, neutral)

> codescout's design is informed by research on compound error in multi-agent systems — failure rates of 41–87% in production pipelines drove our choice of single-session skill-based workflows over agent orchestration chains.
> [Read the analysis →](docs/research/multi-agent-context-loss.md)

---

## Section 2: `docs/agents/`

### Files

```
docs/agents/
├── claude-code.md
├── copilot.md
└── cursor.md
```

### Per-file structure (identical across all three)

1. **One-time setup** — MCP registration config block for that agent
2. **Workflow skills** — how to install (copy commands, no internal paths)
3. **Enforcement hook** — routing hook that blocks raw file reads
4. **Verify** — 2 quick checks to confirm working
5. **Day-to-day workflow** — brainstorm → plan → execute → review → finish (brief summary, links to manual for depth)

### Sanitization rules for copilot.md and cursor.md

- Remove all company/employer/client names and internal system names
- Replace internal paths with `path/to/copilot-codescout/`
- Skills referenced by generic workflow names only
- No references to internal agent orchestration systems or proprietary tooling
- Do not include source file paths or attribution lines referencing internal directory structures (e.g. no "Adapted from /home/..." lines)
- `HowTo.md` + `MANUAL-copilot.md` → `docs/agents/copilot.md`
- `MANUAL-cursor.md` → `docs/agents/cursor.md`

### Source files (external, to be adapted)

**`docs/agents/claude-code.md`** — no external source; written from scratch covering:
- MCP registration in `~/.claude/settings.json` or `.mcp.json`
- Installing `codescout-companion` (the routing plugin — use this as the primary name; "routing plugin" only as a descriptive label in parentheses on first use)
- Verifying setup (`/mcp` + `list_symbols` test)
- Day-to-day workflow summary with links to manual

**`docs/agents/copilot.md`** — adapted from:
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/HowTo.md` (setup steps)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/MANUAL-copilot.md` (workflow)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/copilot-instructions.md` (enforcement)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Skills/` (skills list)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Agents/` (code-reviewer agent)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Hooks/` (enforcement hooks)

**`docs/agents/cursor.md`** — adapted from:
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/MANUAL-cursor.md`

The cursor source covers setup, skills/rules, workflow, and cursor-specific notes — sufficient for sections 1–2 and 5 of the per-file template. For sections 3 (enforcement hook) and 4 (verify), adapt from the copilot source: the Hooks and verify steps are agent-agnostic and apply equally to Cursor. Do not invent content — copy and adapt from the copilot guide where the cursor source has no equivalent section.

---

## Section 3: `docs/research/`

### Files

```
docs/research/
└── multi-agent-context-loss.md
```

### Source file

`/home/marius/work/stefanini/AI-enablement/research/multi-agent-context-loss.md`

### Sanitization rules

- Remove all company/employer/client names and internal system names (replace with `[internal tool]` or generic descriptions like "orchestration-heavy pattern")
- **Sections safe to keep verbatim:** Executive Summary, §1 (Mathematics of Compound Error), §2 (Telephone Game Effect), §3 (Empirical Evidence), §4 (Distributed Information Problem), §5 (Context Rot), §6 (Counter-Argument: When Multi-Agent Works), Sources
- **Sections to rewrite:** §7 (Implications for Our Architecture) — rename to "Implications for codescout's Design", remove internal system descriptions, replace with: why codescout uses single-session workflows + persistent memory instead of agent chains. §8 (Recommendations) — rewrite to remove internal context, keep general guidance.
- **Result:** ~80–90% of the document survives verbatim. Only §7 and §8 need rewriting.
- The key stats (41–87% failure rates, 0.95^N tables) are in the safe sections — they survive and are valid to reference from the README.

---

## Section 4: `docs/manual/` additions

### SUMMARY.md additions

Exact SUMMARY.md diff (two insertions):

```diff
 # User Guide
 
+- [Why codescout?](why-codescout.md)
+
 - [Installation](getting-started/installation.md)
   - [Your First Project](getting-started/first-project.md)
   - [Routing Plugin](getting-started/routing-plugin.md)
 
+- [Agent Integrations](agents/overview.md)
+  - [Claude Code](agents/claude-code.md)
+  - [GitHub Copilot](agents/copilot.md)
+  - [Cursor](agents/cursor.md)
+
 - [Progressive Disclosure](concepts/progressive-disclosure.md)
```

`Agent Integrations` is a top-level chapter inserted between Installation and Progressive Disclosure — same indentation level as both.

`Why codescout?` appears before Installation intentionally — it answers "why should I install this?" before "how do I install it". `Agent Integrations` appears after Installation — it assumes the reader has installed codescout and wants to wire it to their agent.

### New pages

- `docs/manual/src/agents/overview.md` — ~20 lines: one paragraph explaining codescout works with any MCP-capable agent, then a table with 3 rows (Agent | Setup guide | Notes) linking to the 3 sub-pages. No research callout here — that lives in `why-codescout.md`. Written directly (not an include).
- `docs/manual/src/agents/claude-code.md` — `{{#include}}` only, no content: `{{#include ../../../agents/claude-code.md}}`
- `docs/manual/src/agents/copilot.md` — `{{#include}}` only, no content: `{{#include ../../../agents/copilot.md}}`
- `docs/manual/src/agents/cursor.md` — `{{#include}}` only, no content: `{{#include ../../../agents/cursor.md}}`
- `docs/manual/src/why-codescout.md` — ~25 lines, 3 sections:
  1. **The problem** (2–3 sentences) — agents using raw file tools burn context on navigation overhead; link to the comparison table in the README rather than duplicating it
  2. **Design choices** (3–4 sentences) — single-session + persistent memory instead of agent chains; LSP navigation instead of file reads; compact-by-default output
  3. **Research** (1–2 sentences + link) — "These choices are informed by research on compound error in multi-agent systems. [Read the analysis →](../../research/multi-agent-context-loss.md)"

**Single source of truth:** `docs/agents/` is canonical. The 3 manual agent files contain only the `{{#include}}` directive — no duplicated content. Include paths are relative to the file containing the directive: from `docs/manual/src/agents/`, `../../../agents/` resolves to `docs/agents/` (up: src/agents → src → manual → docs). Verify with `mdbook build` after adding includes.

### No other manual changes

The existing SUMMARY.md structure is good. Only the two new chapters are added.

---

## Implementation Order

1. Create directories: `docs/agents/`, `docs/research/`, `docs/manual/src/agents/`
2. Sanitize and write `docs/research/multi-agent-context-loss.md` — verify stats survive before writing README
3. Write `docs/agents/claude-code.md`, `docs/agents/copilot.md`, `docs/agents/cursor.md`
4. Write README (callout stats confirmed present in research doc)
5. Write manual pages: `why-codescout.md`, `agents/overview.md`, `{{#include}}` stubs
6. Update `docs/manual/src/SUMMARY.md`
7. Run `mdbook build` — fix any errors

---

## Definition of Done

- [ ] `mdbook build` succeeds with zero errors and zero warnings (all `{{#include}}` paths resolve)
- [ ] README renders correctly on GitHub (comparison table, links all valid)
- [ ] `docs/agents/copilot.md` and `docs/agents/cursor.md` contain no company/employer/client names — grep check: no matches for internal names
- [ ] `docs/research/multi-agent-context-loss.md` contains no company/employer/client names — same grep check
- [ ] All links in README and agent guides resolve (no 404s)
- [ ] `docs/manual/src/agents/*.md` files (except overview.md) contain only `{{#include}}` — no duplicated content
- [ ] `docs/manual/src/agents/` contains exactly 4 files (`ls docs/manual/src/agents/ | wc -l` → `4`): `overview.md`, `claude-code.md`, `copilot.md`, `cursor.md`

---

## Out of Scope

- Redesigning the mdBook theme or navigation
- Changing existing manual pages (beyond SUMMARY.md additions)
- Adding new tools documentation
- Restructuring `docs/plans/` or `docs/superpowers/`

---

## Files Created / Modified

| File | Action |
|---|---|
| `README.md` | Rewrite (~80 lines) |
| `docs/agents/` | Create directory |
| `docs/research/` | Create directory |
| `docs/manual/src/agents/` | Create directory |
| `docs/agents/claude-code.md` | Create |
| `docs/agents/copilot.md` | Create (adapted + sanitized) |
| `docs/agents/cursor.md` | Create (adapted + sanitized) |
| `docs/research/multi-agent-context-loss.md` | Create (sanitized copy) |
| `docs/manual/src/SUMMARY.md` | Modify (add 2 chapters) |
| `docs/manual/src/why-codescout.md` | Create (~20 lines: design philosophy + research link) |
| `docs/manual/src/agents/overview.md` | Create (~20 lines: intro paragraph + 3-row agent table) |
| `docs/manual/src/agents/claude-code.md` | Create |
| `docs/manual/src/agents/copilot.md` | Create |
| `docs/manual/src/agents/cursor.md` | Create |
