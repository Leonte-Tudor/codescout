# Protected Memories — Design Spec

**Date:** 2026-03-11
**Status:** Draft
**Branch:** experiments

## Problem

When `onboarding(force=true)` runs, it overwrites all 6 onboarding memories
(`project-overview`, `architecture`, `conventions`, `development-commands`,
`domain-glossary`, `gotchas`). User-curated content — especially in `gotchas` —
is lost with no merge or confirmation.

## Goal

Certain memory topics survive force re-onboarding via a hybrid flow:
anchor-based staleness check (fast, deterministic) + LLM verification of stale
entries + user approval before writing.

## Design

### Configuration

New field in `project.toml` under `[memory]`:

```toml
[memory]
protected = ["gotchas"]
```

- `protected`: `Vec<String>` of topic names that onboarding must not blindly
  overwrite.
- **Default:** `["gotchas"]` — set in `MemorySection::default()` so new
  projects get it out of the box.
- Users can add any topic name, including custom ones they created manually.

**Implementation:** Add `protected: Vec<String>` to the existing `MemorySection`
struct in `src/config/project.rs`.

### Rust Changes (workflow.rs — onboarding `call()`)

After the language/file scan but **before** writing memories (~line 870), a new
step gathers protected-memory state:

1. Read `protected` list from `config.memory.protected`.
2. For each protected topic that already has content in `MemoryStore`:
   - Read the existing memory content via `memory.read(topic)`.
   - Load the anchor sidecar (if it exists) and compute staleness via
     `check_path_staleness()`.
3. Bundle into the onboarding result JSON as a new top-level field:

```json
{
  "protected_memories": {
    "gotchas": {
      "exists": true,
      "content": "# Gotchas & Known Issues\n...",
      "staleness": {
        "fresh_files": ["src/embed/index.rs"],
        "stale_files": [
          {
            "path": "src/tools/output_buffer.rs",
            "old_hash": "abc123",
            "new_hash": "def456"
          }
        ],
        "untracked": true
      }
    }
  }
}
```

4. For protected topics that don't exist yet: `"exists": false`.
5. The two programmatic memories (`onboarding`, `language-patterns`) remain
   unchanged — they are machine-generated and always overwritten. If a user
   adds them to `protected`, they are silently excluded.

**Key principle:** The Rust code **computes** staleness but does **not** write
protected memories. It hands structured data to the LLM and lets the prompt
orchestrate the merge + user approval.

### Prompt Changes (onboarding_prompt.md — Phase 2)

Phase 2 ("Write the 6 Memories") gains a conditional flow. Before writing each
memory, check if it appears in `protected_memories` from the onboarding result:

#### Protected + all anchors fresh → skip

Keep as-is. Tell the user:
> "Kept `gotchas` unchanged (all references still valid)."

#### Protected + stale or untracked → hybrid merge flow

1. Read the existing content from `protected_memories[topic].content`.
2. For entries referencing stale files: read the relevant source files and
   verify whether each entry is still accurate.
3. Identify new gotchas discovered during Phase 1 exploration.
4. Present a diff-style summary to the user:
   - **Stale (recommend removing):** [entries no longer accurate]
   - **Still valid (keeping):** [verified entries]
   - **New findings:** [discoveries from fresh exploration]
   - **Proposed merged version:** [full content]
5. **Wait for user approval** before calling `memory(action="write")`.

#### Protected + doesn't exist → create fresh

No existing content to protect. Write as today.

#### Unprotected → overwrite as today

No change in behavior.

### Edge Cases

| Scenario | Behavior |
|---|---|
| First onboarding (no memories) | All protected topics have `exists: false` — created fresh |
| Custom topic in `protected` that onboarding doesn't write | Harmless — Rust reports staleness, prompt never writes it |
| User removes a topic from `protected` | Onboarding overwrites it freely |
| `onboarding` or `language-patterns` in `protected` | Silently excluded — always programmatic |
| No anchor sidecar for a protected memory | `untracked: true` — LLM verifies all entries |

## Files to Change

| File | Change |
|---|---|
| `src/config/project.rs` | Add `protected: Vec<String>` to `MemorySection` with default |
| `src/tools/workflow.rs` | Gather protected-memory state, include in onboarding JSON |
| `src/prompts/onboarding_prompt.md` | Add conditional merge/approve flow in Phase 2 |

## Out of Scope

- **Hard write protection in `MemoryStore`:** Direct `memory(action="write")`
  calls outside onboarding are not blocked. Protection is at the decision
  layer, not the storage layer.
- **Semantic memory protection:** Only markdown topic memories are covered.
  `remember`/`recall`/`forget` are unaffected.
- **Non-onboarding memory writes:** If the user or LLM explicitly writes to a
  protected topic via the memory tool, that's intentional and allowed.
