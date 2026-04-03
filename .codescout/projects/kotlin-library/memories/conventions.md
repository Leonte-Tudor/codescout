# kotlin-library — Conventions

## Language Patterns

### Kotlin Features (by design)
This fixture explicitly exercises modern Kotlin language features for LSP/AST testing:
- **data class** — `Book` with auto-generated equals/hashCode/toString/copy
- **sealed class** — `SearchResult` for exhaustive when expressions
- **value class** — zero-overhead wrapper types in Advanced.kt
- **companion object** — factory methods on `Book`
- **object declaration** — singleton pattern
- **enum class** — `Genre` with member function
- **extension functions** — `Catalog<T>.searchAsync`, `Book.toSearchText`
- **delegated properties** — `by lazy`, `by observable`
- **coroutines** — `suspend` extension on Catalog
- **scope functions** — `let`, `run`, `apply`, `also`

### Naming Conventions
- Classes/interfaces: PascalCase (`Book`, `Catalog`, `Searchable`, `SearchResult`)
- Enum values: UPPER_SNAKE_CASE (`FICTION`, `NON_FICTION`)
- Functions/methods: camelCase (`searchText`, `isAvailable`, `createDefaultCatalog`)
- Properties: camelCase (`copiesAvailable`, `totalItems`)
- Constants: SCREAMING_SNAKE_CASE or `const val` in companion objects

### Testing
- No test files — this is a syntax/structure fixture for codescout's own test suite
- All correctness validation is in codescout's `tests/symbol_lsp.rs` and `tests/integration.rs`

### Build
- Gradle Kotlin DSL (`build.gradle.kts`)
- Single module, `kotlin("jvm")` plugin
- Kotlin stdlib only, no external dependencies
- Kotlin 2.1.0, JVM target
