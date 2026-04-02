## Project Map

| Project | Language | Purpose |
|---------|----------|---------|
| **code-explorer** | Rust | The core MCP server — 29 tools for symbol navigation, semantic search, file ops, memory, shell |
| **java-library** | Java 21 | Test fixture exercising records, sealed interfaces, pattern matching for AST/LSP validation |
| **kotlin-library** | Kotlin 2.1 | Test fixture exercising data classes, sealed classes, coroutines, value classes for AST/LSP validation |
| **python-library** | Python 3.10+ | Test fixture exercising dataclasses, ABC, Protocol, generics, MRO for AST/LSP validation |
| **rust-library** | Rust 2021 | Test fixture exercising structs, enums, traits, lifetimes, generics for AST/LSP validation |
| **typescript-library** | TypeScript ES2022 | Test fixture exercising discriminated unions, decorators, overloads, mapped types for AST/LSP validation |

## Cross-Project Dependencies

The 5 fixture libraries have **no code dependencies** on each other or on code-explorer.
code-explorer depends on the fixtures only at test time:
- `tests/integration.rs` — uses fixture projects via `project_with_files()` temp copies
- `tests/symbol_lsp.rs` — feature-gated (`e2e-rust`, `e2e-python`, `e2e-kotlin`, `e2e-java`, `e2e-typescript`) tests that start real LSP servers against the fixture source trees at `tests/fixtures/<lang>-library/`

## Shared Infrastructure

- **Workspace root:** `Cargo.toml` (Rust workspace) + fixture projects under `tests/fixtures/`
- **CI:** `cargo fmt && cargo clippy -- -D warnings && cargo test` on code-explorer; fixtures are built/checked as part of this
- **Embedding index:** single `project.db` at `.codescout/embeddings/` covers all workspace files
- **Memory:** per-project memories in `.codescout/memory/<project-id>/`, workspace memories in `.codescout/memory/`

## Shared Domain Model

All 5 fixtures implement the **same domain**: a library catalog with Book, Genre, Searchable, Catalog, and SearchResult types. This enables cross-language comparison of how codescout handles equivalent constructs in different languages.