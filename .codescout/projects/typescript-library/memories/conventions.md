# typescript-library — Conventions

## Language Patterns
- **Strict TypeScript**: `strict: true` — no implicit any, strict null checks
- **Private fields via constructor shorthand**: `constructor(private _field: T)` pattern used in `Book`
- **Accessor methods, not properties**: `book.title()` not `book.title` — method-style accessors
- **Underscore prefix for private fields**: `_title`, `_isbn`, `_genre`, `_copiesAvailable`
- **String enums**: `Genre` uses string literal values (`'fiction'`, `'non_fiction'`, etc.)
- **Readonly by default**: `DefaultCatalog.name` uses `readonly`; `ReadonlyBook` utility type

## Naming Conventions
- Classes: PascalCase (`Book`, `Catalog`, `CatalogStats`, `BookService`)
- Interfaces: PascalCase (`Searchable`, `FoundResult`, `BookIndex`)
- Enums: PascalCase values (`Genre.Fiction`, `Genre.NonFiction`)
- Functions: camelCase (`genreLabel`, `createDefaultCatalog`, `isFound`, `findBook`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Type aliases: PascalCase (`SearchResult`, `ReadonlyBook`, `IsAvailable`)
- Private fields: `_camelCase` prefix

## TypeScript Feature Usage
- Discriminated unions with `kind` literal field for result types
- Type guards (`result is FoundResult`) for safe narrowing
- Generic constraints (`T extends Searchable`) on `Catalog<T>`
- Conditional types (`IsAvailable<T>`) and mapped types (`ReadonlyBook`)
- Index signatures (`BookIndex[isbn: string]: Book`)
- Function overloads (`findBook` with two call signatures)
- Decorator syntax (`@logged` on `BookService.process`)
- Namespace + interface merging (`BookMetadata` interface and namespace)
- Default export (`export default class DefaultCatalog`)
- Barrel re-exports via `src/index.ts`

## Testing
- No test files exist — this is a fixture for codescout's own test suite, not a tested library
- codescout tests in `tests/` (parent repo) exercise symbol navigation on this fixture

## Module Conventions
- All imports use relative paths (`'../models/book'`, `'./genre'`)
- `src/index.ts` is the explicit public barrel — only exports what consumers should use
- `types.ts` and `extensions/advanced.ts` are internal/advanced — not re-exported from barrel
- CommonJS module output (`"module": "commonjs"` in tsconfig)
