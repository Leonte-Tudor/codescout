# typescript-library — Project Overview

## Purpose
A TypeScript fixture library that models a book catalog system. It serves as a test fixture
for codescout's TypeScript/LSP tooling — designed to cover a wide range of TypeScript language
features rather than implement production logic.

## Tech Stack
- Language: TypeScript (strict mode, ES2022 target)
- Module system: CommonJS
- Build: tsc (outDir: dist, rootDir: src)
- Compiler flags: experimentalDecorators, emitDecoratorMetadata, strict
- No runtime dependencies; no test framework

## Structure
```
src/
  index.ts               — public re-export barrel
  models/
    book.ts              — Book class, MAX_RESULTS constant
    genre.ts             — Genre enum, genreLabel() function
  interfaces/
    searchable.ts        — Searchable interface (searchText, relevance)
    types.ts             — SearchResult union, result interfaces, utility types
  services/
    catalog.ts           — Catalog<T> generic class, CatalogStats, createDefaultCatalog()
  extensions/
    advanced.ts          — TypeScript feature showcase: overloads, decorators, namespaces, default export
```

## Public API (src/index.ts exports)
- `Book`, `MAX_RESULTS` — from models/book
- `Genre` — from models/genre
- `Searchable` — from interfaces/searchable
- `Catalog`, `createDefaultCatalog` — from services/catalog
- (types.ts and extensions/advanced.ts are NOT re-exported from index.ts)

## Key Facts
- No tests exist (fixture only)
- No README
- Domain: library catalog (books, genres, search)
- `MAX_RESULTS = 100` is the only runtime configuration constant
