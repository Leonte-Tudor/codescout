> ⚠ Experimental — may change without notice.

# Auto-Register Cargo Dependencies on `activate_project`

When a Rust project is activated, codescout now automatically registers its
direct Cargo dependencies as libraries — so `find_symbol`, `list_symbols`, and
`semantic_search` can search inside them without any manual setup.

## What happens

After a successful `activate_project` call on a directory that contains a
`Cargo.toml`, codescout:

1. Parses `[dependencies]` (skips `[dev-dependencies]` and
   `[build-dependencies]`).
2. Looks up each dependency in `~/.cargo/registry/src/index.crates.io-*/`
   (the local Cargo source cache).
3. Registers any crate that is not already in the library registry, using the
   highest available version.
4. Persists the registry to `.codescout/libraries.json` as usual.

Registration is **best-effort**: if a crate is not present in the local
registry (e.g. it has not been fetched yet) or if any step fails, activation
still succeeds and the failure is silently skipped.

## Response shape

When at least one library was newly registered, the response includes an extra
field:

```json
{
  "status": "ok",
  "activated": { ... },
  "auto_registered_libs": ["serde", "tokio", "anyhow"]
}
```

If nothing was newly registered the field is omitted.

## Using registered libraries

Once registered, the normal library scope selectors work:

```
list_symbols(scope="lib:serde")
find_symbol("Deserialize", scope="lib:serde")
semantic_search("async runtime spawn", scope="lib:tokio")
```

Run `index_project(scope="lib:NAME")` to build a semantic search index for
a crate (the auto-registration step does not index — it only makes the crate
visible to the symbol tools).

## Limitations

- Only top-level `[dependencies]` are scanned. Workspace-level deps in the
  root `Cargo.toml` `[workspace.dependencies]` table are picked up; path and
  git deps that have no local registry copy are silently skipped.
- Hyphen/underscore normalisation follows Cargo conventions (`my-crate` and
  `my_crate` are treated as the same name).
- The feature requires that `cargo fetch` (or `cargo build`) has already
  populated `~/.cargo/registry/src/` for the relevant crates.

## Upgrade path

Requires codescout ≥ 0.4.1 (server-side change only; no companion plugin
update needed).
