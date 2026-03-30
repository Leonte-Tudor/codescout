# python-library — Conventions

## Language & Tooling

- Python 3.10+ (uses `list[T]` built-in generics, union syntax)
- No runtime dependencies; stdlib only
- `pyproject.toml` for project metadata (no build backend configured)

## Naming

- Classes: PascalCase (`Book`, `AudioBook`, `Catalog`)
- Private attributes: leading underscore (`_items`, `_name`)
- Private helper functions: leading underscore (`_score`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Type aliases: PascalCase (`BookList`)
- TypeVars: single uppercase letter (`T`)

## Code Style

- All public functions and methods have docstrings
- `@property` used for computed boolean attributes (`is_available`)
- Dunder methods (`__repr__`, `__eq__`, `__hash__`) defined together at end of class
- `@abstractmethod` always paired with `...` as body (not `pass`)

## Testing Note

This is a **fixture**, not a tested library. There are no test files in this project.
All correctness validation is done by codescout's integration tests operating on this
fixture from the outside (symbol extraction, LSP, semantic search).
