# typescript-library — Project Overview

## Purpose

Test fixture for codescout's TypeScript/JavaScript language support. Models a library/book
catalog domain to exercise a wide range of TypeScript language features that codescout's
tree-sitter and LSP integrations must handle correctly.

**Not a standalone application** — no runtime, no tests, no build scripts. Consumed by
codescout's integration and symbol_lsp test suites.

## Tech Stack

- **Language:** TypeScript (ES2022 target, CommonJS modules)
- **Compiler config:** `tsconfig.json` with `strict: true`, `experimentalDecorators: true`
- **No dependencies** — `package.json` has no `dependencies` or `devDependencies`

## Key Files

| File | Role |
|------|------|
| `src/index.ts` | Barrel re-exports: Book, MAX_RESULTS, Genre, Searchable, Catalog, createDefaultCatalog |
| `src/models/book.ts` | `Book` class, `MAX_RESULTS` constant |
| `src/models/genre.ts` | `Genre` enum (string values), `genreLabel` function |
| `src/interfaces/searchable.ts` | `Searchable` interface (searchText, relevance) |
| `src/interfaces/types.ts` | Discriminated union types, type guards, mapped/conditional types |
| `src/extensions/advanced.ts` | Advanced features: overloads, decorators, namespaces, default export |
| `src/services/catalog.ts` | Generic `Catalog<T extends Searchable>` class, `CatalogStats` |
| `package.json` | Minimal — name, version, main entry |
| `tsconfig.json` | Compiler options |
