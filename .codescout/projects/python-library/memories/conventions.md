# python-library — Conventions

## Language Patterns

- **`from __future__ import annotations`** — used in all modules except interfaces and genre
  (enables PEP 604 union syntax and forward refs)
- **Type annotations everywhere** — all function params, return types, and class fields are annotated
- **Docstrings on all public classes/methods** — triple-quoted, single-line for simple descriptions
- **Module-level constants** — typed with `: int = 100` style (not bare assignment)
- **No error handling** — fixture code has no try/except/raise; all paths are happy paths

## Naming Conventions

- Classes: PascalCase (`Book`, `Catalog`, `AudioBook`)
- Functions: snake_case (`search_books`, `rank_results`, `create_default_catalog`)
- Constants: UPPER_SNAKE (`MAX_RESULTS`, `FICTION`, `NON_FICTION`)
- Private members: single underscore prefix (`_items`, `_name`, `_score`)
- Type aliases: PascalCase (`BookList`)
- TypeVars: single uppercase letter (`T`)

## Design Patterns

- **Interface segregation**: `Searchable` (ABC) for behavior, `HasISBN` (Protocol) for structure
- **Generics with bounds**: `Catalog[T]` where `T: Searchable` — ensures search_text() exists
- **Mixin pattern**: `Playable` is a mixin added to `AudioBook` via multiple inheritance
- **Factory function**: `create_default_catalog()` — free function returning configured Catalog
- **Nested class**: `Catalog.Stats` for grouping related data
- **Closure**: `rank_results._score` as sorting key function

## Testing

No test files exist within this project. It IS a test fixture — codescout's Rust
test suite in `src/` and `tests/` exercises this project's symbols, structure,
and cross-file relationships.

## File Organization

- One primary class per file (book.py has Book, genre.py has Genre)
- `__init__.py` re-exports key types from subpackages
- `extensions/` isolates advanced/edge-case constructs from the core domain model
