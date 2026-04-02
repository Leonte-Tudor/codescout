# python-library — Project Overview

## Purpose

Test fixture for the codescout workspace. Provides a small but well-structured
Python library that exercises Python-specific language features for codescout's
AST parser, symbol navigation, and LSP integration tests.

## Tech Stack

- **Language:** Python 3.10+
- **Build:** pyproject.toml (minimal, no external dependencies)
- **Type system:** dataclasses, Enum, ABC, Protocol, Generic, TypeVar
- **No runtime dependencies** — stdlib only

## What It Exercises

This fixture is designed to test codescout's handling of Python-specific constructs:
- `@dataclass` with `@property` methods
- `Enum` subclasses with methods
- Abstract base classes (`ABC` + `@abstractmethod`)
- Structural typing (`Protocol` with `@runtime_checkable`)
- Generics (`Generic[T]` with bounded `TypeVar`)
- Multiple inheritance / MRO (`AudioBook(Book, Playable)`)
- Type aliases (`BookList = list[Book]`)
- Nested classes (`Catalog.Stats`)
- Nested functions / closures (`rank_results._score`)
- `*args` / `**kwargs` signatures (`search_books`)
- Module-level constants with type annotations (`MAX_RESULTS: int = 100`)

## Key Exports (library/__init__.py)

`Book`, `Genre`, `Searchable`, `Catalog` — the four public types.
