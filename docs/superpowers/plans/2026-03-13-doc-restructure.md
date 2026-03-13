# Documentation Restructure Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure codescout's docs into a beginner-friendly README (~80 lines), standalone per-agent setup guides, a sanitized multi-agent research doc, and two new mdBook chapters.

**Architecture:** README becomes a billboard (TLDR + comparison + quick-start + agent links + research callout). Agent guides live in `docs/agents/` as the single source of truth; `docs/manual/src/agents/` uses `{{#include}}` to pull them in. Research doc is a sanitized copy of internal research with company-identifying sections rewritten.

**Tech Stack:** Markdown, mdBook (`mdbook build` to verify)

**Spec:** `docs/superpowers/specs/2026-03-13-doc-restructure-design.md`

---

## Chunk 1: Scaffold + Research Doc

### Task 1: Create directories

**Files:**
- Create: `docs/agents/` (directory)
- Create: `docs/research/` (directory)
- Create: `docs/manual/src/agents/` (directory)

- [ ] **Step 1: Create the three new directories**

```bash
mkdir -p docs/agents docs/research docs/manual/src/agents
```

- [ ] **Step 2: Verify they exist**

```bash
ls -d docs/agents docs/research docs/manual/src/agents
```

Expected output:
```
docs/agents
docs/manual/src/agents
docs/research
```

> **Note:** git does not track empty directories. No commit needed here — the directories will be populated in later tasks and committed then. Proceed to Task 2.

---

### Task 2: Sanitize and write research doc

**Source:** `/home/marius/work/stefanini/AI-enablement/research/multi-agent-context-loss.md`
**Output:** `docs/research/multi-agent-context-loss.md`

**Sanitization rules:**
- Sections §1–§6 and Sources: **keep verbatim** (no company references there)
- §7 "Implications for Our Architecture": **rewrite** — replace internal system names and company references with generic descriptions
- §8 "Recommendations": **keep** but remove any "we/our" framing tied to internal context
- Remove the agent tree diagram with internal names; replace with generic example

**Files:**
- Create: `docs/research/multi-agent-context-loss.md`

- [ ] **Step 1: Copy the safe sections verbatim**

Create `docs/research/multi-agent-context-loss.md` with the following content. Copy §1–§6 and Sources **exactly** from the source file, then write §7–§8 as specified below:

````markdown
# Context Loss and Compound Error in Multi-Agent LLM Systems

> Research compiled March 2026. Sources include peer-reviewed papers (arXiv),
> industry engineering blogs, and empirical benchmarks.

---

## Executive Summary

The intuition is correct: when multiple AI agents each lose ~20% of context or accuracy,
the compound error rate grows exponentially — not linearly. A 5-agent chain where each
agent is 80% accurate yields ~33% end-to-end accuracy (0.80^5 = 0.328), not 80%.
Peer-reviewed research confirms failure rates of 41–87% in production multi-agent systems,
and a 50-percentage-point accuracy gap between single agents with full context vs.
multi-agent systems with distributed information.

This has direct implications for choosing between multi-agent orchestration (agent tree
patterns) and single-session skill-based workflows (the codescout approach).

---

## 1. The Mathematics of Compound Error

### The 0.95^N Problem

If each agent in a sequential chain has 95% accuracy, the system accuracy after N steps is:

| Agents in chain | System accuracy |
|-----------------|-----------------|
| 1               | 95.0%           |
| 3               | 85.7%           |
| 5               | 77.4%           |
| 10              | 59.9%           |
| 15              | 46.3%           |

**At 80% per-agent accuracy (the user's ~20% loss assumption):**

| Agents in chain | System accuracy |
|-----------------|-----------------|
| 1               | 80.0%           |
| 3               | 51.2%           |
| 5               | 32.8%           |
| 7               | 21.0%           |
| 10              | 10.7%           |

The intuition of ">80% error" with multiple agents losing ~20% each is
mathematically validated: five agents at 80% accuracy each produce ~67% error rate.

**Source:** [Why Multi-Agent AI Fails: The 0.95^10 Problem](https://www.artiquare.com/why-multi-agent-ai-fails/)

### Why It's Worse Than Simple Multiplication

Errors don't just pass through — they *amplify*. An error at step 3 corrupts the input
to step 4, which amplifies the error at step 5. By step 8, you're not debugging a model —
you're debugging chaos. This is because each agent makes decisions based on the
(potentially flawed) output of the previous agent, creating a "composition crisis."

**Source:** [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)

---

## 2. The Telephone Game Effect

### The Term

Christopher Yee coined "The Agentic Telephone Game" to describe how AI output layered
on AI output without human checkpoints introduces compounding drift — small accuracy gaps
that accumulate while the output remains fluent and confident.

### The Math

Assuming each AI interaction preserves ~92% accuracy, four rounds of AI-on-AI processing
degrades accuracy to ~72%, even though the output *looks* like it's at 100%. The danger
is that unlike manual work where errors are visible, AI output "decouples effort from
quality" — the text reads beautifully while the facts drift.

### Real-World Example

A strategy document passed through 5 LLM iterations (draft → revise → review → finalize,
each via LLM). Result: "a document that read beautifully, flowed logically" but was built
on unverified foundational assumptions. The entire document was scrapped.

### Speed Makes It Worse

- **Human telephone game:** Takes days, allowing course-correction
- **Agent telephone game:** Runs in minutes with zero opportunity for organic correction

**Source:** [The Agentic Telephone Game: Cautionary Tale](https://www.christopheryee.org/blog/agentic-telephone-game-cautionary-tale/)

---

## 3. Empirical Evidence: Multi-Agent Failure Rates

### Cemri et al. (2025) — "Why Do Multi-Agent LLM Systems Fail?"

The most comprehensive study to date analyzed 5 popular multi-agent frameworks and found:

- **Failure rates: 41% to 86.7%** across 7 state-of-the-art open-source systems
- **14 distinct failure modes** organized into 3 categories
- Tactical improvements (prompt refinement, topology redesign) yielded only **14%
  performance gains** — insufficient for production use

**Failure mode taxonomy (MASFT):**

| Category | Share | Key failures |
|----------|-------|-------------|
| Specification & system design | 41.8% | Role disobedience, conversation history loss, step repetition |
| Inter-agent misalignment | 36.9% | Conversation resets, task derailment, ignoring peer input |
| Task verification gaps | 21.3% | Premature termination, incomplete verification |

Critical finding: **"Conversation history loss"** — agents experience "unexpected context
truncation, disregarding recent interaction history and reverting to antecedent
conversational state." This is the telephone game at the architectural level.

**Source:** [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/html/2503.13657v1)
(arXiv 2503.13657)

### Production Failure Rates (Augment Code analysis)

- **41–86.7% of multi-agent LLM systems fail in production**, with most breakdowns
  occurring within hours of deployment
- **79% of problems** originate from specification and coordination issues, not
  technical implementation
- PwC case study: structured architecture improved code generation accuracy from
  **10% to 70%** (7x), but only with independent validation at each step

**Source:** [Why Multi-Agent LLM Systems Fail and How to Fix Them](https://www.augmentcode.com/guides/why-multi-agent-llm-systems-fail-and-how-to-fix-them)

---

## 4. The Distributed Information Problem

### Collective Reasoning Failures (arXiv 2505.11556)

A 2025 study on collective reasoning in multi-agent LLMs under distributed information found:

- **Up to 50 percentage-point accuracy gap** between single agents with full information
  vs. multi-agent systems with distributed information
- Failure modes: information silos, echo chambers, premature consensus, herding behavior
- Herding: agents converge on incorrect answers because they follow the majority —
  even when individual agents know better

**Source:** [Systematic Failures in Collective Reasoning under Distributed Information](https://arxiv.org/html/2505.11556v3)
(arXiv 2505.11556)

---

## 5. Context Rot: The Underlying Mechanism

### Chroma Research — "Context Rot"

Research from Chroma demonstrates that LLMs do not process context uniformly. As input
length increases, reliability declines — even on simple tasks:

- **Lower semantic similarity** between question and answer causes steeper performance decline
- **Distractors amplify** degradation at scale (non-uniformly — some are far worse)
- **Structured text performs worse** than shuffled/incoherent content (attention mechanisms
  misallocate focus based on structural patterns)
- Claude models abstain when uncertain; GPT models generate confident but incorrect answers

**Implication for multi-agent systems:** Each handoff reconstructs context in a new window.
The receiving agent gets a summary/instruction, not the original reasoning. This is
context rot applied at every boundary.

**Source:** [Context Rot: How Increasing Input Tokens Impacts LLM Performance](https://research.trychroma.com/context-rot)

### JetBrains Research — Context Management for Agents

Empirical study on 500 SWE-bench instances comparing context management strategies:

- Both context management approaches cut expenses by **>50%** vs unmanaged contexts
- **Observation masking matched or exceeded LLM summarization** in 4 of 5 configurations
- Summarization caused agents to run **13–15% longer** (obscured stopping signals)
- Summary generation consumed **>7% of total costs** while performing no better

**Key finding:** Sophisticated summarization sometimes backfires by obscuring signals,
paying more for equivalent or worse results. Simpler approaches (masking) often win.

**Source:** [Cutting Through the Noise: Smarter Context Management](https://blog.jetbrains.com/research/2025/12/efficient-context-management/)

---

## 6. The Counter-Argument: When Multi-Agent Works

### Anthropic's Own Multi-Agent System

Anthropic built a multi-agent research system and published findings:

- Multi-agent with Claude Opus 4 lead agent **outperformed single-agent by 90.2%**
  on internal evaluations
- But: agents use **~4x more tokens than chat**, and multi-agent uses **~15x more tokens**
- Architecture mitigates telephone game by having subagents "store work in external
  systems, then pass lightweight references back to the coordinator"
- Explicit memory mechanisms save research plans to external storage when approaching
  200K token limits

**Critical design choice:** Rather than passing full context through the chain (telephone
game), they pass *references* to externally stored artifacts. The coordinator never
reconstructs another agent's full reasoning — it reads the output artifact directly.

**Source:** [Anthropic Engineering: Multi-Agent Research System](https://www.anthropic.com/engineering/multi-agent-research-system)

### Hybrid Approaches (arXiv 2505.18286)

Research shows the benefits of multi-agent systems over single-agent **diminish as LLM
capabilities improve**. A hybrid cascading design:

- Routes easy requests to single-agent, hard requests to multi-agent
- Improves accuracy by **1.1–12%** while reducing costs by **up to 20%**
- Key insight: "the benefits of MAS over SAS diminish as LLM capabilities improve"

**Source:** [Single-agent or Multi-agent Systems? Why Not Both?](https://arxiv.org/abs/2505.18286)

---

## 7. Implications for codescout's Design

### Orchestration-Heavy Patterns

A common pattern in enterprise AI tooling uses deep delegation trees for complex
development tasks. For example, a 7-agent dependency upgrade workflow might look like:

```
OrchestratorAgent
├── ScannerAgent          (sub-agent)
├── ResearcherAgent       (sub-agent)
├── ImpactAnalyzerAgent   (sub-agent)
├── TestStrategistAgent   (sub-agent)
├── IntentAgent           (sub-agent)
├── PlannerAgent          (sub-agent)
└── CoderAgent            (sub-agent)
```

This is a 7-agent chain. At 90% per-agent accuracy: 0.90^7 = **47.8% system accuracy**.
At 85%: 0.85^7 = **32.1%**. Each sub-agent gets a compressed summary of the orchestrator's
intent — classic telephone game topology.

### Single-Session Skill-Based Workflows

codescout's companion workflow uses **skills, not agents** for the core development loop:

```
brainstorming → writing-plans → subagent-driven-development → finishing
```

Skills execute in the **same context window** as the main session. No inter-agent
handoff = no telephone game. The only delegation is to focused sub-agents for
isolated tasks (code review), where context loss is bounded because the sub-agent
receives the specific artifact (diff/file) rather than a summary of prior reasoning.

### Why Skills Beat Agent Trees for Context Preservation

| Factor | Multi-agent tree | Single-session skills |
|--------|-----------------|----------------------|
| Context continuity | Broken at every handoff | Preserved across skills |
| Error compounding | Multiplicative (0.9^N) | Additive (single session) |
| Information loss | Summary/instruction at each boundary | Full conversation history |
| Telephone game risk | High (N boundaries) | Minimal (0–1 boundaries) |
| Token cost | ~15x chat baseline | ~4x chat baseline |
| Debugging | Distributed across agents | Single conversation trace |
| When better | Truly independent parallel tasks | Sequential reasoning chains |

### When to Use Sub-Agents

Sub-agents are appropriate when:
1. The task is **truly independent** (code review of a specific file)
2. The sub-agent receives the **actual artifact**, not a summary of reasoning
3. The result is **verifiable** (tests pass/fail, lint clean/dirty)
4. Context isolation is a **feature** (preventing contamination of main session)

Following Anthropic's pattern: pass *references to artifacts*, not reconstructed context.

---

## 8. Recommendations

1. **Default to single-session skills** for sequential workflows. The compound error
   math is unforgiving — every handoff is a potential 5–20% accuracy loss.

2. **Use sub-agents only for isolated, verifiable tasks** where the input is a concrete
   artifact (file, diff, test suite) and the output is boolean or structured.

3. **Never pass summaries of reasoning between agents.** Pass references to stored
   artifacts (files, plans, test results). This is what Anthropic does internally.

4. **Add human-in-the-loop checkpoints** at workflow boundaries (brainstorming → plan,
   plan → implementation). Each checkpoint resets the accuracy baseline.

5. **Measure end-to-end accuracy**, not per-agent accuracy. A system of five 90%-accurate
   agents is a 59%-accurate system, not a 90%-accurate one.

---

## Sources

### Peer-Reviewed / arXiv

- Cemri, Pan, Yang et al. — [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/html/2503.13657v1) (arXiv 2503.13657, 2025)
- [Systematic Failures in Collective Reasoning under Distributed Information in Multi-Agent LLMs](https://arxiv.org/html/2505.11556v3) (arXiv 2505.11556, 2025)
- [Single-agent or Multi-agent Systems? Why Not Both?](https://arxiv.org/abs/2505.18286) (arXiv 2505.18286, 2025)
- [Memory Management and Contextual Consistency for Long-Running Low-Code Agents](https://arxiv.org/pdf/2509.25250) (arXiv 2509.25250, 2025)

### Industry Research

- Anthropic Engineering — [Building a Multi-Agent Research System](https://www.anthropic.com/engineering/multi-agent-research-system)
- Chroma Research — [Context Rot: How Increasing Input Tokens Impacts LLM Performance](https://research.trychroma.com/context-rot)
- JetBrains Research — [Cutting Through the Noise: Smarter Context Management for LLM-Powered Agents](https://blog.jetbrains.com/research/2025/12/efficient-context-management/)

### Practitioner Analysis

- Christopher Yee — [The Agentic Telephone Game: Cautionary Tale](https://www.christopheryee.org/blog/agentic-telephone-game-cautionary-tale/)
- Artiquare — [Why Multi-Agent AI Fails: The 0.95^10 Problem](https://www.artiquare.com/why-multi-agent-ai-fails/)
- Towards Data Science — [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)
- Augment Code — [Why Multi-Agent LLM Systems Fail and How to Fix Them](https://www.augmentcode.com/guides/why-multi-agent-llm-systems-fail-and-how-to-fix-them)
- Galileo AI — [Why Do Multi-Agent LLM Systems Fail](https://galileo.ai/blog/multi-agent-llm-systems-fail)
````

- [ ] **Step 2: Verify sanitization — grep for forbidden strings**

```bash
grep -i "stefanini\|safeediting\|upgradeorchestrator\|upgradescanner\|upgraderesearcher\|upgradeimpactanalyzer\|upgradeteststrat\|upgradeintent\|upgradeplanner\|editmode_coder\|copilot-vs-claudecode\|what stefanini\|what we built" docs/research/multi-agent-context-loss.md
```

Expected: **no output** (zero matches). If any matches appear, fix them before continuing.

- [ ] **Step 3: Verify key stats survive**

```bash
grep -cE "41|86\.7|0\.9\^" docs/research/multi-agent-context-loss.md
```

Expected: output > 0 (stats are present).

- [ ] **Step 4: Commit**

```bash
git add docs/research/multi-agent-context-loss.md
git commit -m "docs: add sanitized multi-agent context loss research

Public-facing version of internal research. §7 rewritten to use generic
agent names and remove company-identifying references. All empirical data,
peer-reviewed sources, and external links preserved verbatim.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Chunk 2: Agent Guides

### Task 3: Write docs/agents/claude-code.md

**Files:**
- Create: `docs/agents/claude-code.md`

- [ ] **Step 1: Create the file**

Write `docs/agents/claude-code.md` with the following H2 sections:

**`## One-Time Setup`**

Prerequisites: Rust toolchain, `cargo install codescout`, binary at `~/.cargo/bin/codescout`.

MCP registration — user-level (recommended), edit `~/.claude/settings.json`:

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

Project-level alternative: `.mcp.json` at project root with the same block.

**`## Workflow Skills`**

Claude Code handles workflow skills differently from Copilot/Cursor — skills are loaded
via the Superpowers plugin system, not manually installed files. No manual skill file
installation is needed; skills activate automatically once the companion plugin is set up.
Link to [Superpowers workflow](../manual/src/concepts/superpowers.md) for details.

**`## Routing Plugin (codescout-companion)`**

The routing plugin (codescout-companion) is a Claude Code plugin that enforces codescout
tool use — it adds a `PreToolUse` hook that blocks `Read`, `Grep`, and `Glob` on source
files and redirects to the appropriate codescout tool.

Install via `claude plugin install codescout-companion` or follow the
[Routing Plugin guide](../manual/src/getting-started/routing-plugin.md) for manual
setup via `~/.claude/settings.json`.

**`## Verify`**

Restart Claude Code, run `/mcp` — confirm `codescout` appears as connected.
Then ask: "What symbols are in src/main.rs?" — Claude should call
`mcp__codescout__list_symbols`, not read the file.

**`## Day-to-Day Workflow`**

codescout injects tool guidance automatically into every session via the MCP system
prompt. For the full disciplined development workflow, link to:
- [Superpowers workflow](../manual/src/concepts/superpowers.md)
- [Tool Reference](../manual/src/tools/overview.md)
- [Progressive Disclosure](../manual/src/concepts/progressive-disclosure.md)

- [ ] **Step 2: Verify file exists**

```bash
ls -la docs/agents/claude-code.md
```

- [ ] **Step 3: Commit**

```bash
git add docs/agents/claude-code.md
git commit -m "docs: add Claude Code agent setup guide

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 4: Write docs/agents/copilot.md

**Sources to read before writing:**
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/HowTo.md`
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/MANUAL-copilot.md`
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/copilot-instructions.md`
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Skills/` (skim skill names)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Agents/` (note agent filename)
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/Hooks/` (note hook filenames)

**Files:**
- Create: `docs/agents/copilot.md`

- [ ] **Step 1: Create the file**

Adapt `HowTo.md` + `MANUAL-copilot.md` into a single guide. Replace all internal paths
with `path/to/copilot-codescout/`. No company/employer names anywhere.

**Required H2 sections (in order):**

**`## One-Time Setup`**

Prerequisites: VS Code (latest), GitHub Copilot subscription, Rust toolchain.

Install: `cargo install codescout`

MCP registration — user-level (recommended), edit `~/.config/Code/User/mcp.json`:

```json
{
  "servers": {
    "codescout": {
      "type": "stdio",
      "command": "codescout",
      "args": ["start"]
    }
  },
  "inputs": []
}
```

> Note: VS Code uses `"servers"` not `"mcpServers"`. Per-project alternative: `.vscode/mcp.json` with the same block.

> VS Code schema validation note (include verbatim from HowTo.md — users hit this): if you see *"Failed to validate tool … array type must have items"*, it is a known schema quirk — other tools still load normally.

Enable Agent Skills: Settings → search `chat.useAgentSkills` → enable.

Add workflow skills to `.github/skills/` (VS Code Copilot discovers skills there):

```bash
# Option A: Symlink (recommended — stays in sync with updates)
# NOTE: do NOT mkdir .github/skills first — ln creates the symlink as the target name
mkdir -p .github
ln -s path/to/copilot-codescout/Skills .github/skills

# Option B: Copy (standalone, no sync)
mkdir -p .github/skills
cp -r path/to/copilot-codescout/Skills/* .github/skills/
```

Add code-reviewer agent:

```bash
mkdir -p .github/agents
cp path/to/copilot-codescout/Agents/code-reviewer.agent.md .github/agents/
```

**`## Enforcement Hook`**

The enforcement hook blocks Copilot from reading source files directly and redirects it
to codescout tools. Requires Python 3.

```bash
mkdir -p .github/hooks
cp path/to/copilot-codescout/Hooks/enforce-codescout.py .github/hooks/
cp path/to/copilot-codescout/Hooks/enforce-codescout.json .github/hooks/
```

**`## Always-On Instructions`**

```bash
cp path/to/copilot-codescout/copilot-instructions.md .github/copilot-instructions.md
```

> If `.github/copilot-instructions.md` already exists, append the codescout section rather than overwriting.

**`## Verify`**

Start a new Copilot Chat session and ask: "What symbols are in src/main.ts?"
Copilot should call `mcp__codescout__list_symbols` rather than reading the file directly.

**`## Day-to-Day Workflow`**

Skills activate automatically when their description matches the request.

| What you say | Skill that activates |
|---|---|
| "I want to add X" / "Let's build Y" | `brainstorming` |
| "Create a worktree" | `using-git-worktrees` |
| "Write the plan" | `writing-plans` |
| "Execute the plan" | `subagent-driven-development` |
| "Review this" / "I finished Task N" | `requesting-code-review` |
| "I'm done" / "All tasks complete" | `finishing-a-development-branch` |

Standard flow: `brainstorming → using-git-worktrees → writing-plans → subagent-driven-development → finishing-a-development-branch`

Brief description of each step (adapt from MANUAL-copilot.md workflow section).

**`## Updating Skills`**

```bash
cd path/to/copilot-codescout
git pull
# Symlink: already up to date. Copy: re-run the cp command.
```

- [ ] **Step 2: Verify sanitization**

```bash
grep -i "stefanini\|safeediting\|/home/\|/work/" docs/agents/copilot.md
```

Expected: **no output**.

- [ ] **Step 3: Commit**

```bash
git add docs/agents/copilot.md
git commit -m "docs: add GitHub Copilot agent setup guide

Adapted from HowTo.md and MANUAL-copilot.md. Sanitized: no company names,
no internal paths, generic path/to/ placeholders throughout.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 5: Write docs/agents/cursor.md

**Sources to read before writing:**
- `/home/marius/work/stefanini/AI-enablement/copilot-codescout/MANUAL-cursor.md`
- `docs/agents/copilot.md` (for enforcement hook + verify — adapt, don't invent)

**Files:**
- Create: `docs/agents/cursor.md`

- [ ] **Step 1: Create the file**

Adapt `MANUAL-cursor.md`. Replace internal paths with `path/to/copilot-codescout/`. No
company/employer names. For `## Enforcement Hook` and `## Verify`, adapt from
`docs/agents/copilot.md` (they are agent-agnostic — just change `.github/` paths to
`.cursor/`).

**Required H2 sections (in order):**

**`## One-Time Setup`**

Install: `cargo install codescout`

MCP registration — project-level (recommended), create `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start"]
    }
  }
}
```

> Note: Cursor uses `"mcpServers"` (with `s`), unlike VS Code's `"servers"`.

Global alternative: Cursor → Settings → Cursor Settings → MCP → Add new server.

Add workflow skills as Cursor Rules in `.cursor/rules/`. Two options:

Option A — convert each skill to `.mdc` format (create `.cursor/rules/<name>.mdc` for each
skill in `path/to/copilot-codescout/Skills/`). Frontmatter: `alwaysApply: false`,
`description:` copied from skill frontmatter. Content: full skill body.

Option B — add `using-superpowers` with `alwaysApply: true` as the entry-point rule.

Add code-reviewer agent:

```bash
mkdir -p .cursor/agents
cp path/to/copilot-codescout/Agents/code-reviewer.agent.md .cursor/agents/
```

**`## Enforcement Hook`**

Adapt from copilot guide — change `.github/hooks/` to `.cursor/hooks/`:

```bash
mkdir -p .cursor/hooks
cp path/to/copilot-codescout/Hooks/enforce-codescout.py .cursor/hooks/
cp path/to/copilot-codescout/Hooks/enforce-codescout.json .cursor/hooks/
```

**`## Verify`**

Start new Agent chat, ask: "What symbols are in src/main.ts?" — agent should call
`mcp__codescout__list_symbols`, not read the file.

**`## Day-to-Day Workflow`**

Identical flow to Copilot: `brainstorming → using-git-worktrees → writing-plans → subagent-driven-development → finishing-a-development-branch`

Same trigger table as copilot.md but with "Rule that activates" column header instead of "Skill".

**`## Cursor-Specific Notes`**

Comparison table: Copilot (`.github/skills/<name>/SKILL.md`, `chat.useAgentSkills: true`)
vs Cursor (`.cursor/rules/<name>.mdc`, `alwaysApply: false`).

MCP config key difference: Cursor uses `mcpServers`, VS Code uses `servers`.

- [ ] **Step 2: Verify sanitization**

```bash
grep -i "stefanini\|safeediting\|/home/\|/work/" docs/agents/cursor.md
```

Expected: **no output**.

- [ ] **Step 3: Commit**

```bash
git add docs/agents/cursor.md
git commit -m "docs: add Cursor agent setup guide

Adapted from MANUAL-cursor.md. Enforcement hook and verify adapted from
copilot guide (agent-agnostic). Sanitized: no internal paths or names.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Chunk 3: README + Manual

### Task 6: Rewrite README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Read the current README**

Note the current tool count (29), language list, contributing/license sections — preserve these.

- [ ] **Step 2: Rewrite README.md**

Replace the entire file. The README must have these sections in order, totalling ≤ 85 lines:

**Title + tagline** (3 lines):
```
# codescout
MCP server giving AI coding agents IDE-grade code intelligence — symbol navigation,
semantic search, persistent memory — optimized for token efficiency.

Works with Claude Code, GitHub Copilot, Cursor, and any MCP-capable agent.
```

**`## What it does`** — 3 bullets: symbol navigation (list: find_symbol, list_symbols,
find_references, goto_definition, replace_symbol, backed by LSP, 9 languages), semantic
search (find code by concept using embeddings), token efficiency (compact by default,
details on demand, never dumps full files).

**`## Why not just read files?`** — comparison table:

| Without codescout | With codescout |
|---|---|
| Agent reads full files to find one function | Navigates by symbol name — zero file reads |
| `grep` returns noise (comments, strings, docs) | `find_references` returns exact call sites |
| Context burns on navigation overhead | Token-efficient by design — compact by default |
| State lost between sessions | Persistent memory across sessions |
| Re-reads same modules from different entry points | Symbol index built once, queried instantly |

**`## Quick start`** — `cargo install codescout`, then the MCP config block:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start"]
    }
  }
}
```

Note that Claude Code uses `~/.claude/settings.json`, Cursor uses `.cursor/mcp.json`,
VS Code uses `~/.config/Code/User/mcp.json` (with `"servers"` key instead of `"mcpServers"`).
Link to full installation guide: `docs/manual/src/getting-started/installation.md`.

**`## Agent integrations`** — table with 3 rows:

| Agent | Guide |
|---|---|
| Claude Code | [docs/agents/claude-code.md](docs/agents/claude-code.md) |
| GitHub Copilot | [docs/agents/copilot.md](docs/agents/copilot.md) |
| Cursor | [docs/agents/cursor.md](docs/agents/cursor.md) |

**`## Multi-agent infrastructure`** — exactly 2 sentences + link:

> codescout's design is informed by research on compound error in multi-agent systems — failure rates of 41–87% in production pipelines drove the choice of single-session skill-based workflows over agent orchestration chains. [Read the analysis →](docs/research/multi-agent-context-loss.md)

**`## Tools (29)`** — one line with category counts, language list, link to tool reference:

`Symbol navigation (9) · File operations (6) · Semantic search (3) · Memory (1) · Library navigation (1) · Workflow (2) · Config (2) · GitHub (5)`

Supported languages: Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, C#, Ruby.

→ [Tool reference](docs/manual/src/tools/overview.md)

**`## Contributing`** and **`## License`** — 2–3 lines each, MIT license.

- [ ] **Step 3: Verify README length**

```bash
wc -l README.md
```

Expected: ≤ 85 lines.

- [ ] **Step 4: Verify all `docs/` links resolve**

```bash
grep -o 'docs/[^)]*' README.md | while read f; do [ -e "$f" ] && echo "OK: $f" || echo "MISSING: $f"; done
```

Expected: all lines show `OK:`.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README as beginner-friendly billboard

~80 lines: elevator pitch, comparison table, quick start, agent integrations
table, multi-agent research callout, tool count summary.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 7: Write manual pages

**Files:**
- Create: `docs/manual/src/why-codescout.md`
- Create: `docs/manual/src/agents/overview.md`
- Create: `docs/manual/src/agents/claude-code.md` (`{{#include}}` only)
- Create: `docs/manual/src/agents/copilot.md` (`{{#include}}` only)
- Create: `docs/manual/src/agents/cursor.md` (`{{#include}}` only)

- [ ] **Step 1: Create why-codescout.md**

```markdown
# Why codescout?

AI coding agents using only raw file tools — `cat`, `grep`, `find` — burn most of their
context window on navigation overhead: reading full files to find one function,
re-reading the same module from different entry points, asking questions they already
answered two tool calls ago.

The result is shallow understanding, hallucinated edits, and constant course-correction.
See the comparison table in the project README for a side-by-side view.

## Design choices

codescout exposes the same information an IDE uses — symbol definitions, references, type
info, git history — through a standard MCP interface. Three choices drive the design:

- **Single-session over agent chains** — skills run in the same context window as the
  main session, avoiding the compound error that accumulates at every inter-agent handoff
- **LSP navigation over file reads** — symbol-level queries are 10–50x more token-efficient
  than reading files, and return structured results rather than noise
- **Compact by default** — every tool defaults to the most useful minimal representation;
  full bodies available on demand via `detail_level: "full"`

## Research

These choices are informed by research on compound error in multi-agent systems —
peer-reviewed studies confirm failure rates of 41–87% in production multi-agent pipelines.

[Read the analysis →](../../research/multi-agent-context-loss.md)
```

- [ ] **Step 2: Create agents/overview.md**

```markdown
# Agent Integrations

codescout works with any MCP-capable coding agent. Once registered as an MCP server,
codescout's system prompt injects automatically into every session, giving the agent
the tool selection rules and iron laws for code navigation.

| Agent | Setup guide | Notes |
|---|---|---|
| Claude Code | [Claude Code](claude-code.md) | Routing plugin available for enforcement |
| GitHub Copilot | [GitHub Copilot](copilot.md) | Skills + enforcement hook for VS Code |
| Cursor | [Cursor](cursor.md) | Cursor Rules equivalent of Copilot Skills |
```

- [ ] **Step 3: Create the three {{#include}} stub files**

`docs/manual/src/agents/claude-code.md`:
```markdown
{{#include ../../../agents/claude-code.md}}
```

`docs/manual/src/agents/copilot.md`:
```markdown
{{#include ../../../agents/copilot.md}}
```

`docs/manual/src/agents/cursor.md`:
```markdown
{{#include ../../../agents/cursor.md}}
```

- [ ] **Step 4: Verify the include stubs contain ONLY the include directive**

```bash
cat docs/manual/src/agents/claude-code.md
cat docs/manual/src/agents/copilot.md
cat docs/manual/src/agents/cursor.md
```

Each file should contain exactly one line.

- [ ] **Step 5: Commit**

```bash
git add docs/manual/src/why-codescout.md docs/manual/src/agents/
git commit -m "docs: add why-codescout page and agent integration manual pages

why-codescout.md: design rationale + research link (~25 lines)
agents/overview.md: intro + 3-row agent table
agents/*.md: {{#include}} stubs pointing to docs/agents/ (single source of truth)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 8: Update SUMMARY.md and verify mdbook build

**Files:**
- Modify: `docs/manual/src/SUMMARY.md`

- [ ] **Step 1: Apply the SUMMARY.md diff**

Exact changes to make in `docs/manual/src/SUMMARY.md`:

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

`Why codescout?` is a top-level item before Installation (answers "why?" before "how?"). `Agent Integrations` is a top-level item after Installation, at the same indentation level as `Progressive Disclosure`.

- [ ] **Step 2: Verify SUMMARY.md looks correct**

```bash
head -25 docs/manual/src/SUMMARY.md
```

Expected output (first 25 lines):
```
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
```

- [ ] **Step 3: Run mdbook build**

```bash
cd docs/manual && mdbook build 2>&1
```

Expected: exits with code 0, no `ERROR` lines. If errors appear, check `{{#include}}` paths and SUMMARY.md entries.

- [ ] **Step 4: Verify agents directory has exactly 4 files**

```bash
ls docs/manual/src/agents/ | wc -l
```

Expected: `4`

```bash
ls docs/manual/src/agents/
```

Expected: `claude-code.md  cursor.md  copilot.md  overview.md`

- [ ] **Step 5: Final sanitization check across all new files**

```bash
grep -ri "stefanini\|safeediting\|/home/marius\|upgradeorchestrator" \
  docs/agents/ docs/research/ docs/manual/src/agents/ docs/manual/src/why-codescout.md README.md
```

Expected: **no output**.

- [ ] **Step 6: Commit**

```bash
git add docs/manual/src/SUMMARY.md
git commit -m "docs: update SUMMARY.md with Why codescout? and Agent Integrations chapters

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Final Verification Checklist

Run these after all tasks complete:

```bash
# 1. mdbook builds cleanly
cd docs/manual && mdbook build && echo "BUILD OK"

# 2. All README doc/ links resolve
grep -o 'docs/[^)]*' README.md | while read f; do [ -e "$f" ] && echo "OK: $f" || echo "MISSING: $f"; done

# 3. No forbidden strings in any new public doc
grep -ri "stefanini\|safeediting\|/home/marius" docs/agents/ docs/research/ README.md

# 4. {{#include}} stubs are stubs only
for f in docs/manual/src/agents/claude-code.md docs/manual/src/agents/copilot.md docs/manual/src/agents/cursor.md; do
  lines=$(wc -l < "$f")
  [ "$lines" -le 1 ] && echo "OK stub: $f" || echo "NOT A STUB: $f ($lines lines)"
done

# 5. agents/ directory file count
ls docs/manual/src/agents/ | wc -l  # expected: 4
```
