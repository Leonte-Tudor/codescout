# python-library — Architecture

## Package Structure

Single-package layout under `library/`. No external dependencies beyond the Python
standard library (`abc`, `dataclasses`, `enum`, `typing`).

```
library/interfaces/   — Abstract contracts (Searchable ABC, HasISBN Protocol)
library/models/       — Core domain types (Book dataclass, Genre enum)
library/services/     — Business logic (Catalog generic class)
library/extensions/   — Advanced patterns (AudioBook, mixins, type aliases, closures)
```

## Dependency Graph

```
extensions/advanced.py
  → models/book.py (Book, for AudioBook and BookList)
  → interfaces/searchable.py (Searchable, for AudioBook)

services/catalog.py
  → interfaces/searchable.py (Searchable, for TypeVar bound)

models/book.py
  → models/genre.py (Genre)

interfaces/searchable.py
  → (no internal deps; uses abc, typing)
```

## Python Patterns Exercised

| Pattern | Location |
|---------|----------|
| `@dataclass` with `@property` and dunders | `Book` |
| `Enum` subclass with instance method | `Genre` |
| `ABC` + `@abstractmethod` | `Searchable` |
| `@runtime_checkable Protocol` | `HasISBN` |
| `Generic[T]` + `TypeVar(bound=...)` | `Catalog` |
| Nested class | `Catalog.Stats` |
| Multiple inheritance + MRO | `AudioBook(Book, Playable)` |
| Mixin class | `Playable` |
| Type alias | `BookList = list[Book]` |
| `*args` / `**kwargs` | `search_books` |
| Nested function / closure | `_score` inside `rank_results` |
| Free factory function | `create_default_catalog` |
