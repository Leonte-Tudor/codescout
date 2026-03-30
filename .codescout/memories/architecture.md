# Workspace Architecture

## Project Map

| Project | Role | Language |
|---------|------|----------|
| `code-explorer` | Rust MCP server providing IDE-grade code intelligence to LLMs | Rust |
| `java-library` | Test fixture for Java 21 LSP/AST testing | Java 21 |
| `kotlin-library` | Test fixture for Kotlin 2.1.0 LSP/AST testing | Kotlin/JVM |
| `python-library` | Test fixture for Python 3.10+ symbol extraction | Python |
| `rust-library` | Test fixture for Rust 2021 LSP/AST testing | Rust |
| `typescript-library` | Test fixture for TypeScript strict LSP/AST testing | TypeScript |

## Cross-Project Dependencies

```
code-explorer
  ├── tests/fixtures/java-library      (integration + symbol_lsp tests)
  ├── tests/fixtures/kotlin-library    (integration + symbol_lsp tests)
  ├── tests/fixtures/python-library    (integration + symbol_lsp tests)
  ├── tests/fixtures/rust-library      (integration + symbol_lsp tests)
  └── tests/fixtures/typescript-library (integration + symbol_lsp tests)
```

The 5 fixture libraries have **no dependency on each other** and **no dependency on code-explorer**.
code-explorer's Rust test suite (`tests/integration.rs`, `tests/symbol_lsp.rs`) exercises all
5 fixtures to validate LSP, AST, and semantic search behavior across languages.

## Shared Domain Model

All 5 fixture projects implement the **same book-catalog domain** deliberately:
- `Book` — core entity (title, isbn, genre, availability)
- `Genre` — category enum (Fiction, NonFiction, Science, History, Biography)
- `Catalog<T>` — generic service bounded to Searchable
- `Searchable` — interface/trait/ABC defining `search_text()` + default `relevance()`
- `SearchResult` — sum type (Found, NotFound, Error)

This lets codescout tests verify cross-language symbol navigation on equivalent concepts.

## Shared Infrastructure

- **CI**: GitHub Actions — runs `cargo build --release`, `cargo test`, `cargo clippy -- -D warnings`
  on ubuntu/macos/windows matrix from the code-explorer root
- **Fixture builds**: Fixture build files (Cargo.toml, build.gradle, pyproject.toml, tsconfig.json)
  are not invoked by CI directly — they are parsed/analyzed by codescout's LSP integration tests
- **Semantic index**: Single embeddings DB at `.codescout/embeddings/project.db` covers all 6 projects
- **Memory store**: Per-project memories scoped via `project_id` parameter
