## Build & Test (workspace-level)

```bash
# Full quality check (run before every completion)
cargo fmt && cargo clippy -- -D warnings && cargo test

# Release build (required for live MCP testing)
cargo build --release

# Run with diagnostic logging
cargo run --release -- start --diagnostic

# E2E tests (require real LSP servers installed)
cargo test --features e2e-rust
cargo test --features e2e-python
cargo test --features e2e-kotlin
cargo test --features e2e-java
cargo test --features e2e-typescript

# Semantic index
codescout index --project .

# Restart MCP server after code changes
/mcp
```

## Per-Project
Fixture libraries have no independent build commands — they are built/tested as part of code-explorer's CI.