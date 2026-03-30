# java-library Conventions

## Language Patterns

### Java 21 Modern Features (by design)
This fixture explicitly exercises modern Java language features for LSP/AST testing:
- **Records** — `Book`, `Found`, `NotFound`, `Error` are Java records (immutable, auto-generated equals/hashCode/toString)
- **Sealed interfaces** — `SearchResult` uses `sealed ... permits` to restrict subtypes
- **Pattern matching** — `instanceof Found` used in `isMatch()` default method
- **Default interface methods** — `Searchable.relevance()` returns 0.0; overridable
- **Text blocks / Streams** — `items.stream().filter(...).toList()` in Catalog.search()
- **Custom annotations** — `@Indexed` with `@Retention(RUNTIME)` in Advanced.java

### Naming Conventions
- Classes: PascalCase (`Book`, `Catalog`, `BookProcessor`)
- Interfaces: PascalCase, descriptive (`Searchable`, `SearchResult`, `Indexed`)
- Enum values: UPPER_SNAKE_CASE (`FICTION`, `NON_FICTION`)
- Methods: camelCase, verb-first (`isAvailable`, `searchText`, `createDefault`, `processAll`)
- Fields: camelCase (`copiesAvailable`, `totalItems`)
- Constants: UPPER_SNAKE_CASE (`MAX_RESULTS`)
- Packages: lowercase dot-separated (`library.models`, `library.services`)

### Access Modifiers
- Public API: `public` class/interface/method
- Implementation detail: `private final` for fields (Catalog.items, Catalog.name)
- Nested utility: `public static class` for standalone nested types (CatalogStats, BatchResult)
- Instance-bound nested: non-static inner class (ProcessingContext)

### Documentation
- Every public type and method has a Javadoc comment (`/** ... */`)
- "Extension:" prefix in comments marks advanced-feature demonstrations

### Testing Approach
- No test files in this fixture — it exists solely to exercise codescout's LSP/AST tools
- Tests for this library are in the codescout Rust test suite (integration/symbol_lsp tests)
- Expected test patterns: `list_symbols`, `find_symbol(include_body=true)`, `goto_definition`, `find_references`

### Build Convention
- Gradle Groovy DSL (`build.gradle`, not `build.gradle.kts`)
- Single module, no subprojects
- No dependency declarations — stdlib only
- Java 21 source and target compatibility set explicitly
