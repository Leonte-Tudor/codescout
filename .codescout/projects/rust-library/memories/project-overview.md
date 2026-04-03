## Purpose

Test fixture library for the codescout workspace. Provides a representative Rust codebase
that codescout's integration tests, LSP navigation, AST parsing, and semantic search tools
exercise. Models a library catalog domain (books, genres, search).

**Not a standalone application** -- no binary, no main.rs, no tests, no external dependencies.

## Tech Stack

- Rust 2021 edition
- Pure `std` library -- zero external crates
- Library crate only (`[lib]` in Cargo.toml, no `[[bin]]`)

## Key Dependencies

None. The `Cargo.toml` declares only `[package]` metadata.

## Runtime Requirements

None -- this is a compile-time-only fixture. Built by codescout's CI
(`cargo build` / `cargo check`) to validate Rust LSP and tree-sitter parsing.

## Source Layout

```
src/
  lib.rs              -- crate root, 4 pub modules + re-exports
  models/             -- domain types: Book (struct), Genre (enum)
  traits/             -- Searchable trait + impl for Book
  services/           -- Catalog<T: Searchable> generic service + CatalogStats
  extensions/         -- advanced Rust features: SearchResult enum (struct/tuple variants),
                         BookIterator (Iterator impl), lifetime fns, impl Trait returns, re-exports
```
