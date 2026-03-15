# Library Navigation Enhancements

> ⚠ Experimental — may change without notice.

These enhancements extend the stable [Library Navigation](../concepts/library-navigation.md)
baseline. They are available on the `experiments` branch only.

## Per-Library Embedding Databases

Earlier versions stored all embeddings — project code and every registered library — in a
single `.codescout/embeddings.db`. That layout made it impossible to re-index one library
without rebuilding everything.

The new layout splits storage into separate databases:

```
.codescout/
  embeddings/
    project.db          ← your project's code
    lib/
      tokio.db          ← one file per registered library
      serde.db
      reqwest.db
```

The filename for each library is derived from its registered name: `/` and `\` are replaced
with `--` and the result is lowercased (e.g. `@org/pkg` → `org--pkg.db`).

**Migration is automatic.** If an old `embeddings.db` is found, codescout moves its contents
into the new structure the first time the project is opened with the updated binary. No manual
steps required.

To build a library's index for the first time, or to rebuild it after an update:

```json
{ "tool": "index_project", "arguments": { "scope": "lib:tokio" } }
```

## Version Tracking and Staleness Hints

When `index_project(scope="lib:<name>")` runs, codescout reads the project's lockfile
(`Cargo.lock` for Rust, `package-lock.json` for JS/TS) to record the library version that
was indexed. This is stored as `version_indexed` on the registry entry alongside `version`
(the currently installed version).

After a dependency upgrade — for example, bumping `tokio` from `1.37.0` to `1.38.0` —
`semantic_search` will include a `stale_libraries` field in its response:

```json
{
  "results": [...],
  "stale_libraries": [
    {
      "name": "tokio",
      "indexed": "1.37.0",
      "current": "1.38.0",
      "hint": "tokio was updated — run index_project(scope='lib:tokio') to re-index"
    }
  ]
}
```

Staleness is detected by comparing `version` vs `version_indexed`. The hint appears whenever
these differ and both are known. If the lockfile ecosystem is not recognised (a language
without a supported lockfile), version tracking is skipped and no staleness warning is shown.

## Auto-Discovery via `goto_definition` and `hover`

Libraries are discovered automatically — no manual registration needed for the common case.
When `goto_definition` or `hover` resolves a symbol to a path outside the project root,
codescout:

1. Walks up from that path to find the nearest package manifest (`Cargo.toml`,
   `package.json`, `go.mod`, etc.)
2. Infers the library name from the manifest
3. Registers the library root with `DiscoveryMethod::LspFollowThrough`

The library then appears in `list_libraries` output. If you subsequently call
`index_project(scope="lib:<name>")`, the `symbol.rs` code path will also nudge you when a
library was discovered but not yet indexed:

```json
{ "hint": "Library 'tokio' discovered but not indexed. Run index_project(scope='lib:tokio') to enable semantic search." }
```

## Scope Filtering

Once a library is registered, pass `scope` to any navigation or search tool:

| Value | Targets |
|---|---|
| `"project"` (default) | Your project's source code only |
| `"lib:<name>"` | One specific library (e.g. `"lib:tokio"`) |
| `"libraries"` | All registered libraries combined |
| `"all"` | Project + all registered libraries |

Tools that accept `scope`: `find_symbol`, `list_symbols`, `semantic_search`,
`index_project`.

```json
{ "tool": "find_symbol", "arguments": { "pattern": "spawn", "scope": "lib:tokio" } }
```

```json
{ "tool": "semantic_search", "arguments": { "query": "retry with backoff", "scope": "lib:reqwest" } }
```

## Further Reading

- [Library Navigation](../concepts/library-navigation.md) — stable baseline: scope table,
  auto-discovery overview, when to use library navigation
- [LSP Idle TTL Eviction](lsp-idle-ttl.md) — related experimental feature; LSP servers
  powering `goto_definition` auto-discovery are also subject to idle eviction
