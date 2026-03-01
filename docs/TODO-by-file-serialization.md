# TODO: `by_file` overflow field — use JSON array, not object

**Date:** 2026-02-28
**Context:** Progressive Discoverability implementation (see `docs/plans/2026-02-28-progressive-discoverability-design.md`)
**Priority:** Implement during Task 5 of the implementation plan

---

## Problem

The design doc proposes `by_file` as a JSON **object** (`{"file.rs": 30, ...}`). This has a
serialization order problem:

- `serde_json::Map` uses `BTreeMap` by default, which sorts keys **alphabetically**.
- We want keys sorted by **match count descending** (most relevant file first).
- `IndexMap` (insertion-order-preserving) would fix this but adds a new dependency for one field.

## Decision

**Use a JSON array of `{file, count}` objects instead of a map.**

### Before (design doc original):
```json
{
  "overflow": {
    "by_file": {
      "src/z_main.rs": 50,
      "src/a_utils.rs": 3
    }
  }
}
```
Alphabetical BTreeMap order buries the most relevant file at the bottom.

### After (implementation decision):
```json
{
  "overflow": {
    "by_file": [
      {"file": "src/z_main.rs", "count": 50},
      {"file": "src/a_utils.rs", "count": 3}
    ],
    "by_file_overflow": 5
  }
}
```

## Why array is better

1. **No new dependency.** `Vec<(String, usize)>` serializes in-order as an array. No `indexmap`.
2. **Guaranteed order.** Count-descending order survives serialization. The LLM sees the most
   relevant file first — important because Claude Code reads tool output top-down.
3. **Unambiguous semantics.** Arrays naturally represent "sorted list of things." Objects suggest
   key-value lookup where order is incidental.
4. **Forward-compatible.** We can add fields later (e.g., `"kind_distribution": {...}`) without
   changing the array item shape.

## Internal representation

```rust
// In OverflowInfo:
pub by_file: Option<Vec<(String, usize)>>,
pub by_file_overflow: usize,

// In build_by_file:
fn build_by_file(matches: &[Value], cap: usize) -> (Vec<(String, usize)>, usize) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for m in matches {
        if let Some(file) = m["file"].as_str() {
            *counts.entry(file.to_string()).or_default() += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total_files = sorted.len();
    let overflow = total_files.saturating_sub(cap);
    sorted.truncate(cap);
    (sorted, overflow)
}

// In overflow_json:
if let Some(ref bf) = info.by_file {
    obj["by_file"] = json!(
        bf.iter().map(|(f, c)| json!({"file": f, "count": c})).collect::<Vec<_>>()
    );
    if info.by_file_overflow > 0 {
        obj["by_file_overflow"] = json!(info.by_file_overflow);
    }
}
```

## Impact on implementation plan

- **Task 1 (add indexmap):** REMOVE entirely. Not needed.
- **Task 2 (OverflowInfo):** Use `Vec<(String, usize)>` instead of `IndexMap<String, usize>`.
- **Task 5 (by_file computation):** No change to `build_by_file` logic, just the type.
- **Tests:** Assert on array structure, not object keys.
- **server_instructions.md:** Example shows array format.

## Update the implementation plan

The implementation plan at `docs/plans/2026-02-28-progressive-discoverability-impl.md` needs
updating to reflect this decision before execution begins.
