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
        i=$(( i + 1 ))
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
