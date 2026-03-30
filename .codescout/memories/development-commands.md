# Development Commands

See codescout memory `development-commands` (project="code-explorer") for the full command reference.

## Workspace-Level Commands (run from repo root)

### Build & Verify
```bash
cargo build --release          # production binary (required before MCP server picks up changes)
cargo build                    # dev build (faster, not used by MCP server)
```

### Test
```bash
cargo test                     # all unit + integration tests
cargo test --test integration  # integration tests only (exercises all fixture libraries)
cargo test --test symbol_lsp   # LSP symbol tests (exercises fixture LSPs)
cargo test --test bug_regression -- --ignored  # real-LSP regression tests
```

### Quality
```bash
cargo fmt                      # format (required before commit)
cargo clippy -- -D warnings    # lint (required before commit; -D warnings is CI standard)
```

### Before Completing Work
```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
cargo build --release          # then restart MCP server with /mcp to test live
```

### Semantic Index
```bash
codescout index --project .    # rebuild semantic index from CLI
# OR via MCP: index_project(force=true)
```

## Fixture-Specific Commands (for fixture development, rarely needed)

### java-library / kotlin-library
```bash
cd tests/fixtures/java-library && ./gradlew build
cd tests/fixtures/kotlin-library && ./gradlew build
```

### typescript-library
```bash
cd tests/fixtures/typescript-library && npm run build  # tsc
```

### python-library / rust-library
No build step needed — Python is interpreted, rust-library is compiled by codescout's own Cargo workspace during `cargo test`.
