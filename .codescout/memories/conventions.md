## Shared Conventions

### Commit Style
- Conventional commits: `feat(scope):`, `fix(scope):`, `test(scope):`, `docs:`, `chore:`, `build:`
- Scope matches the affected module or project (e.g., `feat(usage):`, `fix(lsp):`)

### Branch Strategy
- `master` — protected, only cherry-picked tested commits
- `experiments` — active development branch, iterate freely
- Cherry-pick to master after: tests pass, clippy clean, MCP verified (`cargo build --release` + `/mcp`)

### Quality Gates
- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion
- Write tools return `json!("ok")` — never echo content
- All tool outputs use 1-indexed line numbers

### Per-Project Conventions
For language-specific patterns, see per-project memories:
- `memory(project="code-explorer", topic="conventions")` — Rust patterns, error handling, testing
- `memory(project="java-library", topic="conventions")` — Java 21 patterns
- `memory(project="kotlin-library", topic="conventions")` — Kotlin patterns
- `memory(project="python-library", topic="conventions")` — Python patterns
- `memory(project="rust-library", topic="conventions")` — Rust fixture patterns
- `memory(project="typescript-library", topic="conventions")` — TypeScript patterns