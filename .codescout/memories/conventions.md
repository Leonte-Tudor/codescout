# Workspace Conventions

## Shared Across All Projects

### Commit & PR Process
- See `CLAUDE.md § Git Workflow` for the full branch strategy and release cycle
- `experiments` branch for all in-progress work; `master` for cherry-picked, tested commits
- All commits must pass: `cargo fmt && cargo clippy -- -D warnings && cargo test`

### CI Rules
- CI enforces clippy `-D warnings` on ubuntu/macos/windows matrix
- `tool-docs-sync` CI job diffs actual MCP tool names against `docs/manual/src/tools/*.md`
  — adding a tool without updating docs **fails CI**

### Fixture Library Conventions (shared pattern across all 5)
- **No tests** inside fixture directories — they exist as external test targets only
- **No external dependencies** — stdlib only in all fixtures
- **Minimal footprint** — each fixture is ~100-150 lines total
- **Domain-isomorphic** — all fixtures implement the same Book/Genre/Catalog/Searchable/SearchResult model

### Domain Naming (cross-language)
| Concept | Java/Kotlin | Python | Rust | TypeScript |
|---------|-------------|--------|------|------------|
| Core entity | `Book` (record/data class) | `Book` (dataclass) | `Book` (struct) | `Book` (class) |
| Category | `Genre` (enum) | `Genre` (Enum) | `Genre` (enum) | `Genre` (string enum) |
| Search contract | `Searchable` (interface) | `Searchable` (ABC) | `Searchable` (trait) | `Searchable` (interface) |
| Service | `Catalog<T>` (generic class) | `Catalog[T]` (Generic) | `Catalog<T>` (generic struct) | `Catalog<T>` (generic class) |
| Result sum type | `SearchResult` (sealed interface) | N/A | `SearchResult` (enum) | `SearchResult` (union) |

## Per-Project Conventions
- code-explorer: See `memory(project="code-explorer", topic="conventions")`
- java-library: See `memory(project="java-library", topic="conventions")`
- kotlin-library: See `memory(project="kotlin-library", topic="conventions")`
- python-library: See `memory(project="python-library", topic="conventions")`
- rust-library: See `memory(project="rust-library", topic="conventions")`
- typescript-library: See `memory(project="typescript-library", topic="conventions")`
