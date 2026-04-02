## Module Structure

```
lib.rs
├── models::book      -- Book struct (title, isbn, genre, copies_available) + accessor methods
├── models::genre     -- Genre enum (Fiction, NonFiction, Science, History, Biography) + label()
├── traits::searchable -- Searchable trait (search_text, relevance) + impl for Book
├── services::catalog  -- Catalog<T: Searchable> (add, search, stats) + CatalogStats + create_default_catalog()
└── extensions
    ├── results        -- SearchResult enum (Found/NotFound/Error variants) + BookIterator (Iterator impl)
    └── advanced       -- BookRef (derive macros), borrow_title (lifetimes), available_titles (impl Trait), re-export alias
```

## Key Abstractions

- **Book** -- core domain entity with title, isbn, genre, availability
- **Genre** -- 5-variant enum with `label()` for display
- **Searchable** -- trait with `search_text() -> String` (required) and `relevance() -> f64` (default 0.0)
- **Catalog<T: Searchable>** -- generic collection with substring-match search via the Searchable trait
- **SearchResult** -- rich enum demonstrating struct variants, tuple variants, and named fields
- **BookIterator** -- manual Iterator impl with associated type `Item = Book`

## Data Flow: Catalog Search

1. `Catalog::new(name)` creates empty catalog
2. `catalog.add(item)` pushes into `Vec<T>`
3. `catalog.search(query)` calls `item.search_text()` on each, filters with `contains(query)`, returns `Vec<&T>`
4. Book's `search_text()` returns `"{title} ({isbn})"` format
5. Book's `relevance()` returns 1.0 if available, 0.5 otherwise

## Design Patterns

- **Trait-based polymorphism**: Searchable trait decouples Catalog from Book
- **Generic constraints**: `Catalog<T: Searchable>` uses trait bounds
- **Default trait methods**: `relevance()` has a default implementation (0.0)
- **Re-exports**: `lib.rs` re-exports core types; `advanced.rs` re-exports Genre as BookGenre

## Rust Feature Coverage (for codescout testing)

This fixture deliberately exercises many Rust language features:
- Structs, enums (with struct/tuple/unit variants), traits, impls
- Generics with trait bounds
- Lifetime annotations (`borrow_title<'a>`)
- `impl Trait` return types (`available_titles`)
- Derive macros (`Debug, Clone, PartialEq`)
- Associated types (`type Item = Book` in Iterator impl)
- Pattern matching (`matches!` macro, `match` expressions)
- Module system (pub mod, pub use, type aliases via re-export)

## Semantic Search Examples

```
semantic_search("lifetime annotations and borrowing", project="rust-library")
semantic_search("iterator pattern and collection operations", project="rust-library")
semantic_search("generic type constraints and bounds", project="rust-library")
```
