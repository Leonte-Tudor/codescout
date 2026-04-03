# java-library Project Overview

## Purpose
A minimal Java library fixture used for testing codescout's symbol navigation and LSP capabilities
on Java/Kotlin codebases. Models a simple library catalog domain (books, genres, search).

## Tech Stack
- **Language:** Java 21
- **Build:** Gradle (plugin: `id 'java'`)
- **Group/Version:** `library:0.1.0`
- **Source compatibility:** Java 21 (uses modern features: records, sealed interfaces, pattern matching)
- **No external dependencies** — stdlib only (java.util.ArrayList, java.util.List, java.lang.annotation)

## Package Structure
- `library.models` — core data types: `Book` (record), `Genre` (enum)
- `library.interfaces` — `Searchable` interface (search contract)
- `library.services` — `Catalog<T>` generic service
- `library.extensions` — advanced Java features: `SearchResult` (sealed interface), `BookProcessor`,
  `Indexed` (custom annotation)

## Key Types
- `Book` — immutable record with title, isbn, genre, copiesAvailable; `isAvailable()` method
- `Genre` — enum with 5 values (FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY) + `label()` formatting
- `Catalog<T extends Searchable>` — generic catalog service with add/search/stats operations
- `SearchResult` — sealed interface with 3 record subtypes (Found, NotFound, Error)
- `Searchable` — interface with `searchText()` + default `relevance()` method

## Notable
- No test directory — this is a codescout test fixture, not a production library
- Intentionally showcases modern Java 21 features for LSP/AST testing: records, sealed interfaces,
  custom annotations, generics with wildcards, anonymous classes, static vs non-static inner classes
