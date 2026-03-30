# kotlin-library — Architecture

## Module Structure

```
src/main/kotlin/library/
  interfaces/
    Searchable.kt       — interface with searchText() + default relevance()
  models/
    Book.kt             — data class with companion object factory methods
    Genre.kt            — enum class with label() method
  services/
    Catalog.kt          — generic Catalog<T: Searchable> + extension functions
  extensions/
    Advanced.kt         — value classes, delegated properties, object declarations, coroutines
    Results.kt          — SearchResult sealed class hierarchy
```

## Key Abstractions

### `Searchable` (interface)
Core search contract. Requires `searchText(): String`; default `relevance(): Double = 0.0`.

### `Book` (data class)
Immutable value type via Kotlin `data class`. Companion object provides factory methods.
`isAvailable()` computed property checks copies > 0.

### `Catalog<T: Searchable>` (generic class)
Generic service bounded to Searchable. Internal `MutableList<T>`.
- `add(item: T)` — appends
- `search(query: String): List<T>` — filters by `searchText().contains(query)`
- `stats()` — returns item count and name
Extension functions on Catalog: `searchAsync` (coroutine), `Book.toSearchText` (receiver fn).

### `SearchResult` (sealed class)
Sealed class hierarchy with 3 subclasses: `Found(book, score)`, `NotFound(query)`, `Error(message, code)`.
Enables exhaustive `when` expressions.

### `Genre` (enum class)
5 values (FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY) + `label()` member function.

## Data Flow

1. `createDefaultCatalog()` → `Catalog<Book>("Main Library")`
2. `catalog.add(Book(title, isbn, genre))` → appends to internal list
3. `catalog.search(query)` → iterates items, calls `item.searchText().contains(query)`, returns `List<T>`

## Advanced Kotlin Patterns (extensions/)
- `value class` — wrapper types with zero-overhead abstraction
- Delegated properties (`by lazy`, `by observable`)
- Scope functions (`let`, `run`, `apply`, `also`)
- Coroutine extension: `suspend fun Catalog<T>.searchAsync(...)`
- `object` declaration — singleton pattern
- `companion object` — static-like factory on `Book`
