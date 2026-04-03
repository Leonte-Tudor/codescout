# python-library — Architecture

## Module Structure

```
library/
  __init__.py          — package root, re-exports Book, Genre, Searchable, Catalog
  models/
    genre.py           — Genre(Enum) with 5 variants + label() method
    book.py            — @dataclass Book with isbn equality, is_available property
  interfaces/
    searchable.py      — Searchable(ABC) interface + HasISBN(Protocol) structural type
  services/
    catalog.py         — Catalog(Generic[T]) collection with search + nested Stats class
  extensions/
    advanced.py        — AudioBook (multiple inheritance), type alias, nested function
```

## Key Abstractions

| Type | Role | File |
|------|------|------|
| `Searchable` | ABC interface — `search_text()` + `relevance()` | interfaces/searchable.py |
| `HasISBN` | Protocol — structural typing check | interfaces/searchable.py |
| `Genre` | Enum — 5 book categories | models/genre.py |
| `Book` | @dataclass — core domain entity, isbn-based equality | models/book.py |
| `Catalog[T]` | Generic collection bound to Searchable | services/catalog.py |
| `AudioBook` | Book + Playable mixin (MRO) | extensions/advanced.py |

## Dependency Graph

```
interfaces/searchable.py  (no deps — leaf)
models/genre.py           (no deps — leaf)
models/book.py            → models/genre.py
services/catalog.py       → interfaces/searchable.py
extensions/advanced.py    → models/book.py, interfaces/searchable.py
```

## Data Flow: Search

1. `Catalog(name)` creates an empty generic collection
2. `catalog.add(item)` appends to `_items: list[T]`
3. `catalog.search(query)` filters items where `query in item.search_text()`
4. Items must implement `Searchable` (ABC bound on TypeVar `T`)

## Semantic Search Examples

```
semantic_search("abstract interface for searching", project_id="python-library")
semantic_search("dataclass book model", project_id="python-library")
semantic_search("generic catalog collection", project_id="python-library")
semantic_search("multiple inheritance mixin", project_id="python-library")
semantic_search("nested class statistics", project_id="python-library")
```
