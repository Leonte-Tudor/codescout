# Beginner Install Docs & Ollama Setup Script — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give beginners a clear, linear install path — Ollama setup script, correct plugin references, and an expanded companion plugin README with troubleshooting.

**Architecture:** Four independent change groups: (1) new bash script, (2) codescout repo doc fixes, (3) new companion-plugin.md page, (4) companion plugin README expansion. No Rust changes — no cargo work beyond final verification.

**Tech Stack:** Bash, Markdown, mdBook (SUMMARY.md)

---

## File Map

| File | Action | What changes |
|---|---|---|
| `scripts/install-ollama.sh` | **Create** | New script: `--check` / `--install` for Ollama + nomic-embed-text |
| `docs/QUICK-START.md` | **Modify** | Fix model name (`mxbai-embed-large` → `nomic-embed-text`) + plugin name |
| `README.md` | **Modify** | Add brief companion plugin reference in Quick Start |
| `docs/manual/src/getting-started/companion-plugin.md` | **Create** | New page: what it does, install, link to full docs |
| `docs/manual/src/SUMMARY.md` | **Modify** | Add companion-plugin.md entry under Getting Started |
| `docs/manual/src/getting-started/routing-plugin.md` | **Modify** | Replace all `code-explorer-routing` → `codescout-companion`, add See Also |
| `/home/marius/work/claude/claude-plugins/codescout-companion/README.md` | **Modify** | Restructure + add Ollama Setup + Troubleshooting sections |

---

## Task 1: `scripts/install-ollama.sh`

**Files:**
- Create: `scripts/install-ollama.sh`

- [ ] **Step 1: Write the script**

```bash
#!/usr/bin/env bash
#
# Check for and install Ollama + pull the nomic-embed-text embedding model.
#
# Usage:
#   ./scripts/install-ollama.sh --check      # report status without installing
#   ./scripts/install-ollama.sh --install    # install ollama if missing, pull model
#
# Platform: Linux (x86_64, aarch64) and macOS (x86_64, arm64).

set -euo pipefail

MODEL="nomic-embed-text"
OLLAMA_HOST="${OLLAMA_HOST:-http://localhost:11434}"

# ── Helpers ──────────────────────────────────────────────────────────────────

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
ok()    { printf '\033[1;32m[ok]\033[0m    %s\n' "$*"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
err()   { printf '\033[1;31m[error]\033[0m %s\n' "$*"; }
skip()  { printf '\033[1;90m[skip]\033[0m  %s\n' "$*"; }

has_cmd() { command -v "$1" &>/dev/null; }

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        *)       err "Unsupported OS: $(uname -s)"; exit 1 ;;
    esac
}

# ── Check ─────────────────────────────────────────────────────────────────────

check_ollama() {
    if has_cmd ollama; then
        ok "ollama $(ollama --version 2>/dev/null | head -1) found at $(command -v ollama)"
        return 0
    else
        warn "ollama not found — run: ./scripts/install-ollama.sh --install"
        return 1
    fi
}

check_model() {
    if ! has_cmd ollama; then
        warn "cannot check model — ollama not installed"
        return 1
    fi
    if ! curl -sf "${OLLAMA_HOST}/api/tags" &>/dev/null; then
        warn "ollama daemon not running — start with: ollama serve"
        return 1
    fi
    if ollama list 2>/dev/null | grep -q "^${MODEL}"; then
        local digest
        digest=$(ollama list 2>/dev/null | grep "^${MODEL}" | awk '{print $2}' | head -1)
        ok "${MODEL} pulled (${digest})"
        return 0
    else
        warn "${MODEL} not pulled — run: ollama pull ${MODEL}"
        return 1
    fi
}

cmd_check() {
    local all_ok=0
    check_ollama || all_ok=1
    check_model  || all_ok=1
    return $all_ok
}

# ── Install ───────────────────────────────────────────────────────────────────

install_ollama() {
    if has_cmd ollama; then
        skip "ollama already installed ($(ollama --version 2>/dev/null | head -1))"
        return 0
    fi

    local os
    os=$(detect_os)
    info "Installing Ollama on ${os}..."

    case "$os" in
        linux)
            curl -fsSL https://ollama.com/install.sh | sh
            ;;
        macos)
            if has_cmd brew; then
                brew install ollama
            else
                err "Homebrew not found. Install from https://brew.sh/ or download Ollama from https://ollama.com"
                exit 1
            fi
            ;;
    esac

    if has_cmd ollama; then
        ok "ollama installed ($(ollama --version 2>/dev/null | head -1))"
    else
        err "ollama installation failed"
        exit 1
    fi
}

ensure_daemon() {
    if curl -sf "${OLLAMA_HOST}/api/tags" &>/dev/null; then
        skip "ollama daemon already running"
        return 0
    fi

    info "Starting ollama daemon..."
    ollama serve &>/dev/null &
    local pid=$!

    local i=0
    while ! curl -sf "${OLLAMA_HOST}/api/tags" &>/dev/null; do
        if (( i >= 30 )); then
            err "ollama daemon did not start within 30s"
            exit 1
        fi
        sleep 1
        (( i++ ))
    done
    ok "ollama daemon started (pid ${pid})"
}

pull_model() {
    if ollama list 2>/dev/null | grep -q "^${MODEL}"; then
        skip "${MODEL} already pulled"
        return 0
    fi

    info "Pulling ${MODEL}..."
    ollama pull "${MODEL}"

    if ollama list 2>/dev/null | grep -q "^${MODEL}"; then
        ok "${MODEL} ready"
    else
        err "${MODEL} pull failed"
        exit 1
    fi
}

cmd_install() {
    install_ollama
    ensure_daemon
    pull_model
    echo
    ok "All done. Add to .codescout/project.toml:"
    printf '  [embeddings]\n  model = "ollama:%s"\n' "${MODEL}"
}

# ── Entry point ───────────────────────────────────────────────────────────────

usage() {
    printf 'Usage: %s --check | --install\n\n' "$0"
    printf '  --check    Report whether ollama and %s are ready\n' "${MODEL}"
    printf '  --install  Install ollama if missing, then pull %s\n' "${MODEL}"
    exit 1
}

case "${1:-}" in
    --check)   cmd_check ;;
    --install) cmd_install ;;
    *)         usage ;;
esac
```

Save this content to `scripts/install-ollama.sh`.

- [ ] **Step 2: Make the script executable**

```bash
chmod +x scripts/install-ollama.sh
```

- [ ] **Step 3: Verify `--check` runs without error (even with ollama absent)**

```bash
./scripts/install-ollama.sh --check; echo "exit: $?"
```

Expected: Two `[warn]` or `[ok]` lines (depending on whether Ollama is installed on your machine). Exit code 0 or 1, no crash.

- [ ] **Step 4: Verify `--install` dry path (no-args shows usage)**

```bash
./scripts/install-ollama.sh; echo "exit: $?"
```

Expected:
```
Usage: ./scripts/install-ollama.sh --check | --install

  --check    Report whether ollama and nomic-embed-text are ready
  --install  Install ollama if missing, then pull nomic-embed-text
exit: 1
```

- [ ] **Step 5: Commit**

```bash
git add scripts/install-ollama.sh
git commit -m "feat(scripts): install-ollama.sh — check/install ollama + nomic-embed-text"
```

---

## Task 2: Fix `docs/QUICK-START.md`

**Files:**
- Modify: `docs/QUICK-START.md`

- [ ] **Step 1: Fix the embedding model name**

In the "Set Up Semantic Search" section, find:

```toml
[embeddings]
model = "ollama:mxbai-embed-large"
```

Replace with:

```toml
[embeddings]
model = "ollama:nomic-embed-text"
```

Also replace the `ollama pull mxbai-embed-large` line:

```bash
ollama pull nomic-embed-text
```

- [ ] **Step 2: Fix the plugin name and add the install-ollama.sh reference**

In the "Install the Routing Plugin" section, replace:

```
/plugin marketplace add mareurs/sdd-misc-plugins
/plugin install code-explorer-routing@sdd-misc-plugins
```

with:

```
/plugin marketplace add mareurs/sdd-misc-plugins
/plugin install codescout-companion@sdd-misc-plugins
```

Also update the settings.json block — replace `"code-explorer-routing@sdd-misc-plugins": true` with `"codescout-companion@sdd-misc-plugins": true`.

- [ ] **Step 3: Add install-ollama.sh hint in Semantic Search section**

After the `ollama pull nomic-embed-text` line, add:

```
# Or use the install script (checks + installs in one step):
./scripts/install-ollama.sh --install
```

- [ ] **Step 4: Commit**

```bash
git add docs/QUICK-START.md
git commit -m "docs: fix model name and plugin name in QUICK-START.md"
```

---

## Task 3: Add companion plugin reference in `README.md`

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add the reference**

In the README's Quick Start section, locate the onboarding callout:

```markdown
> **Onboarding is essential.** Before starting work on a new project, run
> `onboarding()` — ...
> See the [Claude Code integration guide](docs/agents/claude-code.md) for details.
```

Directly after that block, add:

```markdown
> **Tip:** Install the [codescout-companion plugin](docs/manual/src/getting-started/companion-plugin.md)
> to automatically steer Claude toward codescout tools in every session — including subagents.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add codescout-companion plugin reference in README quick start"
```

---

## Task 4: New `docs/manual/src/getting-started/companion-plugin.md`

**Files:**
- Create: `docs/manual/src/getting-started/companion-plugin.md`

- [ ] **Step 1: Write the page**

```markdown
# codescout-companion Plugin

The `codescout-companion` plugin steers Claude Code — and every subagent it spawns — toward
codescout's symbol-aware tools instead of falling back to `grep`, `cat`, and `Read`. It injects
tool guidance at session start, propagates it to subagents, and hard-blocks native file-reading
tools on source files before they execute.

## Install

```
/plugin marketplace add mareurs/sdd-misc-plugins
/plugin install codescout-companion@sdd-misc-plugins
```

Or add to `~/.claude/settings.json`:

```json
{
  "enabledPlugins": {
    "codescout-companion@sdd-misc-plugins": true
  }
}
```

Start a new Claude Code session after installing — the plugin activates automatically.

## Verify

```bash
claude /plugin list
# should show: codescout-companion@sdd-misc-plugins
```

Then start a session and ask Claude which tool it would use to search for a function by name.
It should cite `find_symbol`, not `grep`.

## Full Documentation

For configuration options, hook details, Ollama setup, and troubleshooting, see the
[codescout-companion README](https://github.com/mareurs/sdd-misc-plugins/tree/main/codescout-companion/README.md).
```

- [ ] **Step 2: Commit**

```bash
git add docs/manual/src/getting-started/companion-plugin.md
git commit -m "docs: add companion-plugin.md getting started page"
```

---

## Task 5: Wire companion-plugin.md into SUMMARY.md + fix routing-plugin.md

**Files:**
- Modify: `docs/manual/src/SUMMARY.md`
- Modify: `docs/manual/src/getting-started/routing-plugin.md`

- [ ] **Step 1: Add companion-plugin.md to SUMMARY.md**

In `docs/manual/src/SUMMARY.md`, find the Getting Started block (it appears twice due to a duplicate `# User Guide` section — update **both** occurrences):

```markdown
- [Installation](getting-started/installation.md)
  - [Your First Project](getting-started/first-project.md)
  - [Routing Plugin](getting-started/routing-plugin.md)
```

Replace each occurrence with:

```markdown
- [Installation](getting-started/installation.md)
  - [Your First Project](getting-started/first-project.md)
  - [Routing Plugin](getting-started/routing-plugin.md)
  - [Companion Plugin](getting-started/companion-plugin.md)
```

- [ ] **Step 2: Fix routing-plugin.md — replace all plugin name references**

In `docs/manual/src/getting-started/routing-plugin.md`, replace every occurrence of `code-explorer-routing@sdd-misc-plugins` with `codescout-companion@sdd-misc-plugins`.

Also replace the display name `code-explorer-routing` (without the scope suffix) with `codescout-companion` wherever it appears as a plugin name (not as a section heading like "Why the Plugin Exists").

- [ ] **Step 3: Add "See Also" link at the bottom of routing-plugin.md**

In the "Further Reading" section at the bottom of `routing-plugin.md`, add:

```markdown
- [Companion Plugin (getting started)](companion-plugin.md) — quick install guide for `codescout-companion`
```

- [ ] **Step 4: Commit**

```bash
git add docs/manual/src/SUMMARY.md docs/manual/src/getting-started/routing-plugin.md
git commit -m "docs: wire companion-plugin.md into SUMMARY, fix plugin name in routing-plugin.md"
```

---

## Task 6: Expand companion plugin README

**Files:**
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/README.md`

This task restructures the existing README and adds two new sections. The existing content is preserved; only the order and two additions change.

- [ ] **Step 1: Move "Quick Install" to the top**

After the opening description paragraph (before "## What It Does"), insert a new section:

```markdown
## Quick Install

```
/plugin marketplace add mareurs/sdd-misc-plugins
/plugin install codescout-companion@sdd-misc-plugins
```

Start a new Claude Code session — the plugin activates automatically.
```

- [ ] **Step 2: Move "Requirements" above "What It Does"**

Reorder the sections so the document reads:

1. Description (existing)
2. Quick Install (new, added in step 1)
3. Requirements (existing — move it here from its current position)
4. What It Does (existing)
5. Installation → rename to "Full Installation" (existing)
6. Configuration (existing)
7. Hooks (existing)
8. Ollama Setup (new — step 3)
9. Troubleshooting (new — step 4)
10. Coupling to codescout (existing)
11. Changelog (existing)

- [ ] **Step 3: Add "Ollama Setup" section**

Insert after the Hooks section:

```markdown
## Ollama Setup

Semantic search (`semantic_search`, `index_project`) requires an embedding backend.
The recommended option is Ollama — fully local, no API key required.

**Using the install script** (from the codescout repo root):

```bash
./scripts/install-ollama.sh --check     # verify current state
./scripts/install-ollama.sh --install   # install ollama + pull nomic-embed-text
```

**Manually** (if you already have Ollama):

```bash
ollama pull nomic-embed-text
```

Then add to `.codescout/project.toml` in your project:

```toml
[embeddings]
model = "ollama:nomic-embed-text"
```

Build the index once in a Claude Code session:

```
Run index_project
```

→ [Embedding backends reference](https://github.com/mareurs/codescout/blob/master/docs/manual/src/configuration/embedding-backends.md)
```

- [ ] **Step 4: Add "Troubleshooting" section**

Insert after Ollama Setup, before "Coupling to codescout":

```markdown
## Troubleshooting

### "codescout not detected"

The plugin scans these locations for a codescout server entry (in order):

1. `.claude/codescout-companion.json` (or `.claude/codescout-routing.json` for backwards compat)
2. `.mcp.json` in the project root
3. `~/.claude/.claude.json`
4. `~/.claude/settings.json`

It matches any server whose `command` or `args` contain `codescout` or `code-explorer`.

If auto-detection fails, force it with `.claude/codescout-companion.json`:

```json
{ "server_name": "codescout" }
```

### Tools not routing to codescout

Verify the plugin is enabled:

```bash
claude /plugin list
# should show: codescout-companion@sdd-misc-plugins
```

Check that `block_reads` is not set to `false` in `.claude/codescout-companion.json`.

### LSP errors on first use (`find_symbol`, `goto_definition` fail)

LSP servers start during `onboarding`. If you skipped it, run:

```
Run onboarding
```

This detects languages, starts LSP servers, and writes project memories. Without it,
symbol navigation tools return errors because no LSP server is running.

### `semantic_search` returns nothing or errors

The embedding index has not been built yet. Run:

```
Run index_project
```

For a ~100k line project this takes 1–3 minutes. Verify status with `project_status`.

If `index_project` fails, confirm Ollama is running:

```bash
curl http://localhost:11434/api/tags
```

### MCP server fails to start (tools missing from Claude Code)

```bash
which codescout        # verify the binary is on PATH
codescout --version    # verify it runs
claude mcp list        # verify it is registered
```

If `codescout` is not on PATH, install it (`cargo install codescout`) or add
`~/.cargo/bin` to your PATH.

### SubagentStart hook not firing

After updating Claude Code, plugins sometimes need to be re-enabled:

```bash
claude /plugin list
```

If `codescout-companion@sdd-misc-plugins` is absent, reinstall:

```
/plugin install codescout-companion@sdd-misc-plugins
```
```

- [ ] **Step 5: Commit from the companion plugin repo**

```bash
cd /home/marius/work/claude/claude-plugins
git add codescout-companion/README.md
git commit -m "docs: restructure README — quick install at top, add Ollama setup + troubleshooting"
```

---

## Task 7: Final verification

- [ ] **Step 1: Run cargo checks from codescout repo root**

```bash
cd /home/marius/work/claude/code-explorer
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Expected: all pass. No Rust code changed — this is a sanity check only.

- [ ] **Step 2: Spot-check the script**

```bash
./scripts/install-ollama.sh --check
```

Expected: `[ok]` or `[warn]` lines, no crash.

- [ ] **Step 3: Verify `nomic-embed-text` is the only model name in install docs**

```bash
grep -r "mxbai-embed-large" docs/ README.md
```

Expected: no output (all instances replaced).

- [ ] **Step 4: Verify no `code-explorer-routing` remains**

```bash
grep -r "code-explorer-routing" docs/ README.md
```

Expected: no output.
