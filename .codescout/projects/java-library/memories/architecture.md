# java-library Architecture

## Module Structure

```
src/main/java/library/
├── models/
│   ├── Book.java          — record (immutable data carrier)
│   └── Genre.java         — enum with label() method
├── interfaces/
│   └── Searchable.java    — search contract (searchText + default relevance)
├── services/
│   └── Catalog.java       — generic catalog with nested CatalogStats
└── extensions/
    ├── Advanced.java      — annotation + BookProcessor (wildcards, anonymous class)
    └── Results.java       — SearchResult sealed interface hierarchy
```

## Key Abstractions

### Searchable (interface)
Core search contract. Any catalog item must implement `searchText() : String`.
Default `relevance() : double` returns 0.0 — override for custom ranking.

### Book (record)
Immutable value type. Fields: title, isbn, genre, copiesAvailable.
Compact constructor defaults copiesAvailable=1.
`isAvailable()` returns `copiesAvailable > 0`.
`MAX_RESULTS = 100` class constant.

### Catalog<T extends Searchable> (generic class)
Generic service bounded to Searchable. Internally holds `List<T> items`.
- `add(T)` — appends to list
- `search(String)` — streams items, filters by `item.searchText().contains(query)`
- `stats()` — returns nested `CatalogStats` (totalItems, name)
- `createDefault()` — static factory returning `Catalog<Book>` named "Main Library"

### SearchResult (sealed interface)
Exhaustive sum type with 3 record variants:
- `Found(Book book, double score)` — match found
- `NotFound(String query)` — no results
- `Error(String message, int code)` — failure
Default `isMatch()` uses `instanceof Found` pattern matching.

### Indexed (@interface)
Custom runtime annotation (`@Retention(RUNTIME)`) with `value()` element defaulting to "".
Used on `BookProcessor.process(Book)` to mark field indexing (isbn).

## Design Patterns
- **Record-based value types** — Book, Found, NotFound, Error are all records (immutable, auto-toString/equals)
- **Sealed interface hierarchy** — SearchResult restricts subtypes for exhaustive switch
- **Generic bounded types** — Catalog<T extends Searchable> enforces the search contract at compile time
- **Static factory** — Catalog.createDefault() as named constructor
- **Static nested class** — CatalogStats (no outer reference needed); non-static ProcessingContext (bound to BookProcessor instance)
- **Anonymous class** — BookProcessor.createAnonymousSearchable() returns inline Searchable impl
- **Wildcard generics** — processAll(List<? extends Searchable>) accepts any Searchable subtype list

## Data Flow: Add and Search
1. `Catalog.createDefault()` → `new Catalog<>("Main Library")`
2. `new Book(title, isbn, genre)` → compact ctor → `this(title, isbn, genre, 1)`
3. `catalog.add(book)` → `items.add(book)`
4. `catalog.search(query)` → stream → filter `searchText().contains(query)` → toList()

## Data Flow: Search Result Handling
1. Search operation returns `SearchResult` (Found/NotFound/Error)
2. `result.isMatch()` → `this instanceof Found`
3. Pattern match: `if (result instanceof SearchResult.Found(var book, var score)) {...}`
4. Error: `SearchResult.Error(var msg, var code)` for graceful failure reporting

## Good semantic_search Queries
- `semantic_search("generic catalog search filtering", project_id="java-library")`
- `semantic_search("sealed interface search result variants", project_id="java-library")`
- `semantic_search("custom annotation retention runtime", project_id="java-library")`
- `semantic_search("book record immutable availability", project_id="java-library")`
- `semantic_search("searchable interface default relevance method", project_id="java-library")`
