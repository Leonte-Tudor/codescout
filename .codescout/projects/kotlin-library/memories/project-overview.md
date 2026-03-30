# kotlin-library — Project Overview

## Purpose
A fixture library project used to test codescout's Kotlin/JVM language intelligence
(AST parsing, LSP, symbol navigation, semantic search). It models a small book catalog
domain — not production software.

## Tech Stack
- **Language:** Kotlin (JVM target, Kotlin 2.1.0)
- **Build:** Gradle with Kotlin DSL (`build.gradle.kts`)
- **Runtime:** JVM via `kotlin("jvm")` plugin
- **Dependencies:** Kotlin stdlib only — no frameworks, no test dependencies

## Project Name & Coordinates
- Root project name: `kotlin-library`
- Gradle group: `library`, version: `0.1.0`

## Structure
```
src/main/kotlin/library/
  interfaces/   Searchable.kt
  models/       Book.kt, Genre.kt
  services/     Catalog.kt
  extensions/   Advanced.kt, Results.kt
```
No test sources (`src/test/`) — this is a syntax/structure fixture, not a tested codebase.

## Key Facts
- 6 source files, ~130 lines total
- The `extensions/` package deliberately exercises advanced Kotlin syntax:
  sealed classes, value classes, delegated properties, scope functions, coroutines, object declarations
- Exists alongside similar fixtures in java-library, python-library, rust-library, typescript-library
  within the codescout test suite
