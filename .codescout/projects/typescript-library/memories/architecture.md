# typescript-library — Architecture

## Module Structure

### models/
- `Book` (class): Core domain object. Private fields `_title`, `_isbn`, `_genre`, `_copiesAvailable`.
  Accessor methods `title()`, `isbn()`, `isAvailable()`, `genre()` — no setters (effectively immutable after construction).
- `Genre` (enum): `Fiction | NonFiction | Science | History | Biography` — string enum values.
  `genreLabel(genre)` converts snake_case enum value to human-readable string.

### interfaces/
- `Searchable` (interface): Contract for catalog items. Requires `searchText(): string`; optional `relevance?(): number`.
- `types.ts` — utility and result types (not in public barrel):
  - `SearchResult = FoundResult | NotFoundResult | ErrorResult` — discriminated union on `kind` field
  - `isFound(result)` — type guard returning `result is FoundResult`
  - `ReadonlyBook = Readonly<Pick<Book, 'title' | 'isbn'>>` — mapped/utility type
  - `IsAvailable<T>` — conditional type: `true` if T has `isAvailable(): boolean`
  - `BookIndex` — index signature: `[isbn: string]: Book`

### services/
- `Catalog<T extends Searchable>` (generic class): Holds a list of `T[]`. Methods:
  - `add(item: T): void` — appends to internal array
  - `search(query: string): T[]` — linear filter on `item.searchText().includes(query)`
  - `stats(): CatalogStats` — returns `{ totalItems, name }`
- `CatalogStats` (class): Simple value object with `totalItems: number` and `name: string`.
- `createDefaultCatalog()` — factory returning `Catalog<any>` named `'Main Library'`.

### extensions/advanced.ts (TypeScript feature showcase)
- `findBook` — function overload signatures (by ISBN → `Book | undefined`; by title+author → `Book[]`)
- `logged` — method decorator (returns descriptor unchanged; demonstrates `@decorator` syntax)
- `BookService` — class with `@logged`-decorated `process(book)` method
- `BookMetadata` — interface + namespace merging (declaration merging pattern): namespace adds `create()` factory
- `DefaultCatalog` — `export default class` with readonly `name = 'default'`

## Data Flows

### Add-and-search flow
1. Caller constructs `Catalog<Book>` (or via `createDefaultCatalog()`)
2. Calls `catalog.add(book)` — book appended to `items: T[]`
3. Calls `catalog.search("fiction")` — filters items where `book.searchText().includes("fiction")`
   Note: `Book` does NOT implement `Searchable` in this fixture — the constraint `T extends Searchable`
   means a concrete `Book`-backed catalog would need a wrapper or `Book` to be extended.
4. Returns `T[]` of matching items.

### Search result classification flow
1. A function returns `SearchResult` (union: `FoundResult | NotFoundResult | ErrorResult`)
2. Caller uses `isFound(result)` type guard to narrow to `FoundResult`
3. `result.kind === 'found'` — discriminant field enables exhaustive switch/narrowing
4. On error path: `ErrorResult` carries `message: string` and `code: number`

## Design Patterns
- Generic constraint (`T extends Searchable`) — open for extension without modifying Catalog
- Discriminated union + type guard — ergonomic result handling without exceptions
- Declaration merging (interface + namespace) — `BookMetadata` is both a type and a factory namespace
- Experimental decorators — `@logged` demonstrates TypeScript decorator syntax
- Barrel export — `src/index.ts` controls the public API surface explicitly

## Good semantic_search queries (use project_id="typescript-library")
- `semantic_search("add book to catalog and search", project_id="typescript-library")`
- `semantic_search("discriminated union result type", project_id="typescript-library")`
- `semantic_search("generic constraint Searchable interface", project_id="typescript-library")`
- `semantic_search("TypeScript decorator method", project_id="typescript-library")`
- `semantic_search("namespace declaration merging BookMetadata", project_id="typescript-library")`
