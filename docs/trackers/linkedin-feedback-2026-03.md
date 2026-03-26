# LinkedIn Feedback — March 2026

Actionable items from external feedback on codescout.

## Deferred

### Token efficiency benchmarks
Create first-party before/after measurements comparing context usage with
codescout tools vs native Read/Grep for equivalent tasks. We cite external
research (AgentDiet, SWE-Pruner, MCP-Zero) but have zero quantitative data
from codescout itself. Could instrument token counts from `usage.db` and
compare against native tool baselines.

### Semantic search scaling benchmarks
Document indexing time and search quality at scale. Current pipeline (4-way
concurrent embedding, sqlite-vec KNN) works for small-to-medium codebases but
has no benchmarks for large repos (100k+ files). Test on a few large
open-source repos, measure indexing time by backend (Ollama CPU/GPU, OpenAI),
and document results.

## Active

### Restore HTTP transport (rmcp 1.x migration)
HTTP transport broke during rmcp 1.x migration. Need to: enable
`transport-streamable-http-server` feature in Cargo.toml, update server setup
code for new rmcp API, handle per-connection session model, update docs.
Currently returns a helpful error at runtime. Not urgent — stdio covers all
primary use cases (Claude Code, Gemini CLI). HTTP unlocks remote/shared
deployments and web-based clients.

### Improve onboarding skip behavior
If onboarding is skipped, tools still work but the agent lacks project context.
Improve fallback: auto-detect language from file extensions, infer basic project
structure from build files (Cargo.toml, package.json), provide minimal
navigation hints even without explicit onboarding. Make it feel like a
"strongly recommended first step" rather than a hard requirement.

## Resolved

_(none yet)_
