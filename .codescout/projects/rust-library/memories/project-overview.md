# rust-library — Project Overview

## Purpose
A Rust fixture library used as a test target for codescout's symbol navigation, LSP,
and semantic search features. It models a book catalog system with realistic Rust
patterns: traits, generics, enums, lifetimes, and re-exports.

## Tech Stack
- Language: Rust (edition 2021)
- No external dependencies (Cargo.toml has empty [dependencies])
- Crate type: library (src/lib.rs)

## Module Layout
- `src/lib.rs` — root; declares 4 modules and re-exports core types
- `src/models/` — domain data types (Book, Genre)
- `src/traits/` — trait definitions (Searchable)
- `src/services/` — business logic (Catalog<T>, CatalogStats)
- `src/extensions/` — advanced Rust feature demos (SearchResult enum, BookIterator, BookRef, lifetimes)

## Key Public API (re-exported from lib.rs)
- `Book` — core entity with title, isbn, genre, copies_available
- `Genre` — enum (Fiction, NonFiction, Science, History, Biography)
- `Searchable` — trait for search text + relevance scoring
- `Catalog<T: Searchable>` — generic catalog service

## Runtime Requirements
- No runtime dependencies; pure library
- No tests embedded in source (fixture purpose only)
