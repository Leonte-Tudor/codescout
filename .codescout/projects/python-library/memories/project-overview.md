# python-library — Project Overview

A Python fixture project used by codescout's integration tests. It demonstrates a
library catalog domain implemented in idiomatic Python 3.10+, covering the full
range of Python symbol kinds that codescout's tree-sitter parser must handle.

## Purpose

This fixture is a **test target**, not a production library. Its code is crafted to
exercise symbol extraction, LSP navigation, and semantic search across:

- Abstract base classes (`Searchable`)
- Runtime-checkable protocols (`HasISBN`)
- Dataclasses with properties and dunder methods (`Book`)
- Enums with methods (`Genre`)
- Generic classes with TypeVar (`Catalog[T]`)
- Nested classes (`Catalog.Stats`)
- Mixins and multiple inheritance (`AudioBook(Book, Playable)`)
- Type aliases (`BookList = list[Book]`)
- Free functions, *args/**kwargs, nested functions/closures (`search_books`, `rank_results`)

## Layout

```
library/
  __init__.py
  interfaces/
    searchable.py       — Searchable (ABC), HasISBN (Protocol)
  models/
    book.py             — Book (dataclass), MAX_RESULTS constant
    genre.py            — Genre (Enum)
  services/
    catalog.py          — Catalog[T] (Generic), Catalog.Stats (nested), create_default_catalog()
  extensions/
    advanced.py         — BookList (type alias), Playable (mixin), AudioBook, search_books, rank_results
pyproject.toml          — name="library", version="0.1.0", requires-python=">=3.10"
```

## Key Types

| Symbol | Kind | File | Notes |
|--------|------|------|-------|
| `Book` | dataclass | `models/book.py` | isbn-keyed equality, `is_available` property |
| `Genre` | Enum | `models/genre.py` | 5 values, `label()` method |
| `Searchable` | ABC | `interfaces/searchable.py` | abstract `search_text()`, default `relevance()` |
| `HasISBN` | Protocol | `interfaces/searchable.py` | `@runtime_checkable`, structural typing |
| `Catalog[T]` | Generic class | `services/catalog.py` | TypeVar `T bound=Searchable`, nested `Stats` |
| `AudioBook` | class | `extensions/advanced.py` | Multiple inheritance: `Book + Playable` |
| `BookList` | type alias | `extensions/advanced.py` | `list[Book]` |
