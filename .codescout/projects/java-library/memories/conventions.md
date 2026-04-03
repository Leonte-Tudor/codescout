## Conventions

### Language Patterns (Java 21)
- **Records over classes** for data carriers (`Book`, `Found`, `NotFound`, `Error`)
- **Sealed interfaces** for closed type hierarchies (`SearchResult permits ...`)
- **Default methods** on interfaces for optional behavior (`relevance()`, `isMatch()`)
- **Pattern matching** with `instanceof` (no explicit cast needed)
- **Streams API** for collection operations (`stream().filter().toList()`)

### Naming
- Package: `library.<layer>` (models, interfaces, services, extensions)
- Classes: PascalCase, singular nouns (`Book`, `Catalog`, `BookProcessor`)
- Interfaces: adjective or noun describing capability (`Searchable`, `Indexed`)
- Enums: PascalCase type, SCREAMING_SNAKE members (`Genre.NON_FICTION`)
- Methods: camelCase verbs (`searchText`, `isAvailable`, `createDefault`)
- Constants: SCREAMING_SNAKE (`MAX_RESULTS`)

### Documentation
- Javadoc `/** ... */` on all public types, constructors, and methods
- Comments prefixed with "Extension:" mark features exercising specific Java constructs
  (e.g., "Extension: sealed interface hierarchy", "Extension: anonymous class")

### Build
- Gradle Groovy DSL (`build.gradle`)
- Group: `library`, version: `0.1.0`
- Java 21 source and target compatibility
- No external dependencies — stdlib only

### Project Role
- This is a **codescout test fixture**, not a production codebase
- Each file is designed to exercise specific tree-sitter / LSP parsing scenarios:
  - `Book.java` — records, compact constructors, constants
  - `Genre.java` — enums with methods
  - `Searchable.java` — interfaces, default methods
  - `Catalog.java` — generics with bounds, nested classes, static factories, streams
  - `Advanced.java` — annotations, anonymous classes, wildcards, inner classes
  - `Results.java` — sealed interfaces, record variants, pattern matching
- No tests exist — the fixture is tested by codescout's own test suite
