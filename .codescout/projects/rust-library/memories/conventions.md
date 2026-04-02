## Language Patterns

- **Rust 2021 edition**, no external dependencies
- `snake_case` for functions/methods/fields, `PascalCase` for types/traits/enums
- All public items have `///` doc comments
- Extension features annotated with `/// Extension: <what it demonstrates>` comments

## Naming

- Accessor methods named after the field: `title()`, `isbn()`, `genre()`, `is_available()`
- Constructors: `new()` (associated function), `create_default_catalog()` (free function)
- Trait methods: `search_text()`, `relevance()`

## Code Organization

- One type per file (book.rs, genre.rs, searchable.rs, catalog.rs)
- `mod.rs` files only contain `pub mod` declarations
- `lib.rs` declares modules and re-exports core public types
- Extensions module groups advanced Rust features (lifetimes, iterators, derive, re-exports)

## Code Quality

- `#[derive(Debug, Clone, PartialEq)]` on all value types
- Private fields with public accessor methods (encapsulation)
- Generic types with explicit trait bounds

## Testing

- **No tests in this project.** This is a test fixture for codescout -- the tests that exercise
  this code live in the parent `code-explorer` project (e2e-rust feature flag).
- The fixture is built/checked as part of codescout's CI to verify Rust LSP and tree-sitter integration.
