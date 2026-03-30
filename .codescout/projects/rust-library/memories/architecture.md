# rust-library — Architecture

## Module Structure

```
src/
  lib.rs              — crate root; pub use re-exports for Book, Genre, Searchable, Catalog
  models/
    mod.rs            — pub mod book; pub mod genre
    book.rs           — Book struct + impl (new, title, isbn, is_available, genre); MAX_RESULTS const
    genre.rs          — Genre enum (5 variants) + impl label()
  traits/
    mod.rs            — pub mod searchable
    searchable.rs     — Searchable trait + impl for Book
  services/
    mod.rs            — pub mod catalog
    catalog.rs        — Catalog<T: Searchable> struct + CatalogStats; create_default_catalog()
  extensions/
    mod.rs            — pub mod results; pub mod advanced
    results.rs        — SearchResult enum (Found/NotFound/Error), BookIterator impl Iterator
    advanced.rs       — BookRef struct, borrow_title (lifetime), available_titles (impl Trait), BookGenre re-export
```

## Key Abstractions

### `Searchable` trait (src/traits/searchable.rs)
Interface for anything searchable: `search_text() -> String` (required) and
`relevance() -> f64` (default 0.0). Book implements it: relevance is 1.0 if available, 0.5 otherwise.

### `Catalog<T: Searchable>` (src/services/catalog.rs)
Generic service parameterized over any Searchable type. Holds a `Vec<T>`.
- `add(&mut self, item: T)` — append to catalog
- `search(&self, query: &str) -> Vec<&T>` — filter by `search_text().contains(query)`
- `stats(&self) -> CatalogStats` — total_items + name

### `Book` (src/models/book.rs)
Private fields (title, isbn, genre, copies_available). Constructor + accessor methods.
`is_available()` checks `copies_available > 0`.

### `SearchResult` enum (src/extensions/results.rs)
Mixed struct/tuple/unit variants: `Found { book, score }`, `NotFound(String)`,
`Error { message, code }`. Demonstrates Rust enum expressiveness.

## Data Flow — Typical Search
1. Caller creates `Catalog::<Book>::new(name)` or uses `create_default_catalog()`
2. `catalog.add(book)` pushes Book into items Vec
3. `catalog.search("query")` iterates items, calls `item.search_text()` (trait dispatch),
   filters by substring match, returns `Vec<&Book>`
4. Caller inspects results via Book accessor methods

## Data Flow — Availability Check
1. `Book::is_available()` returns `copies_available > 0`
2. `Searchable::relevance()` for Book returns 1.0 if available, 0.5 if not
3. This allows ranking-aware search consumers to sort by relevance

## Design Patterns
- Trait objects via `Searchable` trait with default method impl
- Generics with trait bounds: `Catalog<T: Searchable>`
- Lifetime annotations in `extensions/advanced.rs`: `borrow_title<'a>(book: &'a Book) -> &'a str`
- `impl Trait` return type: `available_titles(books: &[Book]) -> impl Iterator<Item = &str>`
- Re-exports: `pub use` at crate root and `Genre as BookGenre` in extensions
- Derive macros: `#[derive(Debug, Clone, PartialEq)]` on Genre and BookRef

## Semantic Search Tips (for when index is built)
- "book catalog search implementation" → catalog.rs search method
- "trait implementation for book" → searchable.rs impl block
- "generic struct with trait bound" → Catalog<T: Searchable>
- "enum with struct and tuple variants" → SearchResult in results.rs
- "lifetime annotation function" → borrow_title in advanced.rs
