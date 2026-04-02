# typescript-library — Architecture

## Module Structure

```
src/
  index.ts              — barrel exports (public API surface)
  models/
    book.ts             — Book class (domain entity), MAX_RESULTS constant
    genre.ts            — Genre enum, genreLabel helper
  interfaces/
    searchable.ts       — Searchable interface (trait-like contract)
    types.ts            — type system showcase: unions, guards, mapped, conditional, index sigs
  extensions/
    advanced.ts         — advanced TS features: overloads, decorators, namespaces, default export
  services/
    catalog.ts          — Catalog<T> generic collection, CatalogStats, createDefaultCatalog factory
```

## Key Abstractions

- **`Book`** — Domain entity with private fields and getter methods (title, isbn, genre, isAvailable)
- **`Genre`** — String enum with 5 variants (Fiction, NonFiction, Science, History, Biography)
- **`Searchable`** — Interface contract: `searchText(): string` + optional `relevance(): number`
- **`Catalog<T extends Searchable>`** — Generic collection with add/search/stats operations
- **`SearchResult`** — Discriminated union: `FoundResult | NotFoundResult | ErrorResult` with `kind` discriminant
- **`isFound`** — Type guard narrowing `SearchResult` to `FoundResult`

## TypeScript Features Exercised

This fixture is specifically designed to cover TS features that stress AST/LSP tooling:

| Feature | Location |
|---------|----------|
| Classes with private fields + accessors | `Book` in `models/book.ts` |
| String enums | `Genre` in `models/genre.ts` |
| Interfaces | `Searchable`, `FoundResult`, `NotFoundResult`, `ErrorResult`, `BookIndex` |
| Discriminated unions | `SearchResult` in `interfaces/types.ts` |
| Type guard functions | `isFound` in `interfaces/types.ts` |
| Mapped types | `ReadonlyBook = Readonly<Pick<Book, ...>>` |
| Conditional types | `IsAvailable<T>` |
| Index signatures | `BookIndex` |
| Generics with constraints | `Catalog<T extends Searchable>` |
| Function overloads | `findBook` (3 signatures) in `extensions/advanced.ts` |
| Decorators (experimental) | `@logged` on `BookService.process` |
| Declaration merging (namespace + interface) | `BookMetadata` |
| Default exports | `DefaultCatalog` |
| Barrel re-exports | `index.ts` |

## Data Flow

1. **Catalog search:** `Catalog.add(item)` → pushes to `items[]` → `Catalog.search(query)` filters via `item.searchText().includes(query)`
2. **Result discrimination:** `SearchResult` union → `isFound(result)` type guard narrows to `FoundResult` → access `.book` and `.score`

## Import Graph

```
index.ts → models/book, models/genre, interfaces/searchable, services/catalog
models/book.ts → models/genre
interfaces/types.ts → models/book
extensions/advanced.ts → models/book
services/catalog.ts → interfaces/searchable
```

## Search Tips (for codescout queries scoped to this project)

- `find_symbol("Book", path="tests/fixtures/typescript-library")` — domain entity
- `find_symbol("Catalog", path="tests/fixtures/typescript-library")` — generic collection
- `find_symbol("SearchResult", path="tests/fixtures/typescript-library")` — union type
- `find_symbol("findBook", path="tests/fixtures/typescript-library")` — overload example
- `grep("@logged", path="tests/fixtures/typescript-library")` — decorator usage
