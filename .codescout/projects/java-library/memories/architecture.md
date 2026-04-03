## Architecture

### Module Structure

```
src/main/java/library/
  models/
    Book.java          — record: title, isbn, genre, copiesAvailable; compact constructor; isAvailable()
    Genre.java         — enum: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY; label() formatter
  interfaces/
    Searchable.java    — interface: searchText() + default relevance()
  services/
    Catalog.java       — generic service: Catalog<T extends Searchable>; add/search/stats/createDefault
  extensions/
    Advanced.java      — Indexed annotation + BookProcessor (annotations, anon classes, wildcards, inner classes)
    Results.java       — sealed SearchResult interface with Found/NotFound/Error record variants
```

### Key Abstractions

1. **Searchable** — Core contract. Anything in the catalog implements `searchText()`.
   Default `relevance()` returns 0.0; implementors override for ranking.

2. **Catalog<T extends Searchable>** — Generic container. Holds items in `ArrayList<T>`,
   searches via Java streams (`filter` + `contains`), exposes `CatalogStats` nested class.
   Static factory `createDefault()` creates a `Catalog<Book>`.

3. **SearchResult** — Sealed interface modeling search outcomes as an algebraic type:
   `Found(Book, score)`, `NotFound(query)`, `Error(message, code)`. Default method
   `isMatch()` uses `instanceof` pattern matching.

4. **Book** — Record with 4 components. Compact constructor defaults `copiesAvailable=1`.
   The primary data entity in the domain.

5. **BookProcessor** — Demonstrates advanced patterns: custom `@Indexed` annotation,
   anonymous `Searchable` implementation, wildcard generics, static vs non-static inner classes.

### Data Flows

**Search flow:** `Catalog.add(item)` populates the internal list. `Catalog.search(query)`
streams all items, filters by `item.searchText().contains(query)`, collects to list.
Result typing is decoupled — `SearchResult.Found` wraps a `Book` + score.

**Processing flow:** `BookProcessor.processAll(List<? extends Searchable>)` iterates
over any Searchable subtype, calling `searchText()` on each. Demonstrates wildcard
bounds enabling polymorphic processing.

### Design Patterns
- Algebraic data types via sealed interface + records (SearchResult)
- Strategy pattern via Searchable interface (pluggable search text)
- Factory method (Catalog.createDefault)
- Bounded generics for type-safe containers (Catalog<T extends Searchable>)

### Useful Queries
- `find_symbol("Catalog", project_id="java-library")` — main service class
- `find_symbol("SearchResult", project_id="java-library")` — sealed type hierarchy
- `find_symbol("Searchable", project_id="java-library")` — core interface
- `grep("sealed|record|permits", path="tests/fixtures/java-library")` — Java 21 features
- `grep("stream|filter|toList", path="tests/fixtures/java-library")` — functional data flow
