# rust-library — Conventions

## Language Patterns

### Naming
- Structs/Enums: PascalCase (Book, Genre, Catalog, SearchResult, BookRef, BookIterator)
- Traits: PascalCase (Searchable)
- Functions/methods: snake_case (search_text, is_available, create_default_catalog)
- Constants: SCREAMING_SNAKE_CASE (MAX_RESULTS = 100)
- Type aliases via re-export: `Genre as BookGenre`

### Struct Design
- Fields are private by default; accessor methods expose them
- Constructor always named `new(...)` returning `Self`
- Availability/state checked via predicate methods (`is_available()`)

### Trait Pattern
- Traits define the minimal required interface (`search_text`)
- Default method implementations provided where sensible (`relevance() -> 0.0`)
- Trait impl placed in the traits module (not alongside the struct)

### Enum Usage
- Pure enum for categories: `Genre` with 5 unit variants + `label()` method
- Rich enum for results: `SearchResult` mixes struct variants (`Found { book, score }`),
  tuple variants (`NotFound(String)`), and struct variants (`Error { message, code }`)
- `matches!` macro used for variant checking in `is_match()`

### Advanced Rust Features Demonstrated
- Lifetime annotations: `borrow_title<'a>(book: &'a Book) -> &'a str`
- `impl Trait` return: `available_titles() -> impl Iterator<Item = &str>`
- Custom Iterator: `BookIterator` implements `Iterator` with associated type `Item = Book`
- `#[derive(Debug, Clone, PartialEq)]` on data types

### Module Organization
- Each concept in its own file; `mod.rs` is a thin re-exporter
- Crate root (`lib.rs`) re-exports the 4 core public types for ergonomic imports
- `extensions/` module exists specifically to demonstrate advanced Rust constructs

### Testing
- No embedded `#[cfg(test)]` blocks — this is a pure fixture for external test harnesses
- No integration tests in the fixture itself
