# typescript-library — Conventions

## Language Patterns

- **Strict mode:** `strict: true` in tsconfig — all strict checks enabled
- **Private fields:** constructor parameter properties with `private` keyword (`private _title: string`)
- **Getter methods:** no `get` keyword — plain methods returning private fields (`title(): string`)
- **Naming:** PascalCase for types/classes/enums/interfaces, camelCase for functions/methods/variables
- **Underscore prefix** for private backing fields (`_title`, `_isbn`, `_genre`, `_copiesAvailable`)
- **JSDoc comments** on all public symbols (`/** ... */`)
- **String enum values:** lowercase with underscores (`'fiction'`, `'non_fiction'`)
- **Discriminated unions:** `kind` as the discriminant property with string literal types

## Module Organization

- **Barrel pattern:** `index.ts` re-exports the public API — not all symbols, only the core ones
- **Feature grouping:** `models/` for entities, `interfaces/` for contracts/types, `services/` for logic, `extensions/` for advanced feature demos
- **One concern per file:** each file covers a single class/enum/interface or a related group of types

## TypeScript-Specific Conventions

- **ES2022 target** with CommonJS modules
- **Experimental decorators** enabled (`experimentalDecorators: true`, `emitDecoratorMetadata: true`)
- **Generic constraints** used for type safety (`T extends Searchable`)
- **Type guards** as standalone exported functions (`isFound`)
- **Extension comments:** advanced features are marked with `/** Extension: <feature name>. */` JSDoc

## Testing

No test files in this fixture. It is tested indirectly by codescout's own test suite
(integration tests and symbol_lsp tests that exercise TypeScript parsing and navigation).

## Build & Run

No build or run commands — this is a static fixture. The `tsconfig.json` exists for
LSP/editor tooling and codescout's TypeScript language server, not for compilation.
