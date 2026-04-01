# Beginner Install Docs & Ollama Setup Script — Design

**Date:** 2026-04-01
**Status:** Approved

## Overview

Improve the codescout installation experience for beginners by:
1. Adding `scripts/install-ollama.sh` to automate Ollama + nomic-embed-text setup
2. Fixing stale/inconsistent references in existing docs (plugin name, model name)
3. Adding a new `docs/manual/src/getting-started/companion-plugin.md` page
4. Expanding the companion plugin README into a thorough beginner guide

Approach chosen: **Option B** — codescout stays self-contained, companion plugin README
becomes the canonical deep-dive, codescout docs point to it.

---

## Section 1: `scripts/install-ollama.sh`

Mirrors the `install-lsp.sh` pattern exactly.

### Usage

```
./scripts/install-ollama.sh --check      # report status without installing
./scripts/install-ollama.sh --install    # install ollama if missing, pull nomic-embed-text
```

### `--check` behavior

Reports two facts:
- Is `ollama` on PATH? If yes, print version. If no, warn with install hint.
- Is `nomic-embed-text` pulled? Check via `ollama list | grep nomic-embed-text`.

Output style matches `install-lsp.sh`: colored `[ok]`/`[warn]`/`[error]` lines.

### `--install` behavior

1. Check if `ollama` is on PATH — skip if already installed.
2. Install Ollama:
   - Linux: `curl -fsSL https://ollama.com/install.sh | sh`
   - macOS: `brew install ollama`
3. Start the Ollama daemon if not already running (`ollama serve &`, with a brief
   health-poll loop waiting for the API to respond on port 11434).
4. Pull `nomic-embed-text`: `ollama pull nomic-embed-text`
5. Verify: `ollama list | grep nomic-embed-text` — print `[ok]` or `[error]`.

### Style constraints

- Same color functions as `install-lsp.sh`: `info()`, `ok()`, `warn()`, `err()`, `skip()`
- `set -euo pipefail` at the top
- `detect_os()` / `detect_arch()` helpers for platform branching
- Idempotent: safe to run multiple times

---

## Section 2: Codescout Repo Doc Changes

Four targeted edits.

### 2a. Normalize embedding model to `nomic-embed-text`

- **`docs/QUICK-START.md`**: replace `mxbai-embed-large` → `nomic-embed-text` in the
  Ollama setup block and the `.codescout/project.toml` example.
- **`docs/manual/src/getting-started/installation.md`**: verify all model references say
  `nomic-embed-text` (already correct in main body; check Feature Flags section too).

### 2b. Fix stale plugin name + add README reference

- **`docs/QUICK-START.md`**: in the "Install the Routing Plugin" block, replace
  `code-explorer-routing@sdd-misc-plugins` → `codescout-companion@sdd-misc-plugins`.
- **`README.md`**: the main README has no plugin reference at all. Add a brief mention
  in the Quick Start section (1–2 sentences + link to `docs/manual/src/getting-started/companion-plugin.md`)
  so a reader landing on the repo page knows the companion plugin exists.

### 2c. New page: `docs/manual/src/getting-started/companion-plugin.md`

~40 lines. Structure:
- What the plugin does (2–3 sentences)
- Install options (plugin command / settings.json)
- Link to companion plugin README on GitHub for full deep-dive
- Link to `routing-plugin.md` for the older code-explorer-routing context

### 2d. Wire into the manual

- Add `companion-plugin.md` to `docs/manual/src/SUMMARY.md` under Getting Started,
  after `routing-plugin.md`.
- Add a "See also" link in `routing-plugin.md`'s Further Reading pointing to
  `companion-plugin.md`.

---

## Section 3: Companion Plugin README Expansion

**File:** `/home/marius/work/claude/claude-plugins/codescout-companion/README.md`

### New structure

```
# codescout-companion
[one-line description]

## Quick Install         ← moved to top: 2 commands, done
## Requirements          ← existing content, placed early
## What It Does          ← existing, follows requirements
## Full Installation     ← existing Options 1 & 2 (renamed from "Installation")
## Configuration         ← existing, unchanged
## Hooks                 ← existing table, unchanged
## Ollama Setup          ← NEW: points to scripts/install-ollama.sh + manual one-liner
## Troubleshooting       ← NEW: 5–6 common failures with fixes
## Coupling to codescout ← existing, keep
## Changelog             ← existing, keep at bottom
```

### New: Ollama Setup section

Points to `scripts/install-ollama.sh` as the canonical path. Also shows the manual
one-liner for users who already have Ollama:

```bash
ollama pull nomic-embed-text
```

Explains that this is needed for `semantic_search` and `index_project`, with a link
to codescout's embedding backends doc.

### New: Troubleshooting section

Covers the most common beginner failures:

| Problem | Fix |
|---|---|
| "codescout not detected" | What configs are scanned; how to use explicit config override |
| "tools not routing" | Verify with `claude /plugin list`; check `block_reads` setting |
| "LSP errors on first use" | Must run `onboarding` first; LSP servers start then |
| "semantic_search returns nothing" | Index not built yet; run `index_project` |
| "MCP server fails to start" | Binary not on PATH; run `which codescout` |
| "SubagentStart not firing" | Plugin may need re-enable after Claude Code update |

---

## What is NOT in scope

- No changes to `install-lsp.sh`
- No new LSP language support
- No changes to codescout tool behavior
- No changes to the manual beyond Getting Started section
