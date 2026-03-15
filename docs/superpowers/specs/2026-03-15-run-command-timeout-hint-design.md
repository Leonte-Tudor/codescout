# run_command Timeout Parameter Leniency — Design Spec

**Date:** 2026-03-15
**Status:** Approved
**Scope:** `src/tools/workflow.rs` — `RunCommand::call()` only

---

## Problem

Agents frequently pass `timeout: 120000` (wrong key, millisecond value) instead of
`timeout_secs: 120`. The current parser reads only `input["timeout_secs"]`; any other key
falls through to the `_ => 30` default. The command then silently runs with a 30-second
timeout, which may be far too short (e.g. `cargo publish` takes 60–120 s).

Two failure modes observed:
1. **Wrong key** — agent passes `timeout` or `timeout_ms` instead of `timeout_secs`
2. **Likely milliseconds** — agent passes `timeout_secs: 120000` thinking the unit is ms

---

## Design

### Parse helper: `parse_timeout_input`

Replace the existing 4-line parse block in `RunCommand::call()` with a dedicated function:

```rust
fn parse_timeout_input(input: &Value) -> (u64, Option<String>)
```

Returns `(timeout_secs: u64, hint: Option<String>)`.

### Decision table

| Condition | Resolved seconds | `hint` emitted |
|-----------|-----------------|----------------|
| `timeout_secs` present, value ≤ 3 600 | value as-is | none |
| `timeout_secs` present, value > 3 600 | value / 1 000 | `"timeout_secs: {raw} looks like milliseconds — converted to {converted}s. Use timeout_secs with a value in seconds."` |
| `timeout` present, value < 1 000 | value as-is (already seconds) | `"Unknown parameter 'timeout' — use timeout_secs. Interpreted {value} as seconds."` |
| `timeout` present, value ≥ 1 000 | value / 1 000 | `"Unknown parameter 'timeout' — use timeout_secs. Converted {raw}ms → {converted}s."` |
| Neither key present | 30 (default) | none |

Priority: `timeout_secs` takes precedence over `timeout` when both are present.

### Hint delivery

The hint is attached to the tool result as a top-level `"timeout_hint"` field:

```json
{
  "stdout": "...",
  "stderr": "...",
  "exit_code": 0,
  "timeout_hint": "Unknown parameter 'timeout' — use timeout_secs. Converted 120000ms → 120s."
}
```

This keeps the hint out of `stdout`/`stderr` (which belong to the subprocess) while
ensuring it surfaces in the same response as the command result. The agent sees the
correction nudge alongside the result it wanted.

### No schema changes

`timeout` is intentionally NOT added to the JSON schema. Adding it as a documented
parameter would encourage its use. The hint directs agents to the canonical `timeout_secs`.

---

## Affected Files

| File | Change |
|------|--------|
| `src/tools/workflow.rs` | Replace timeout parse block in `call()` with `parse_timeout_input()`. Add the helper function. Attach `timeout_hint` to the result. |

No changes to `run_command_inner`, the server, or any other file.

---

## Tests

| Test name | What it covers |
|-----------|----------------|
| `parse_timeout_input_correct_key_small` | `timeout_secs: 120` → 120s, no hint |
| `parse_timeout_input_correct_key_large` | `timeout_secs: 120000` → 120s, hint present |
| `parse_timeout_input_wrong_key_small` | `timeout: 300` → 300s, hint present |
| `parse_timeout_input_wrong_key_large` | `timeout: 120000` → 120s, hint present |
| `parse_timeout_input_neither_key` | no key → 30s, no hint |
| `parse_timeout_input_both_keys` | `timeout_secs` wins over `timeout` |

All tests are unit tests on `parse_timeout_input` — no MCP server spin-up needed.

---

## Out of Scope

- `timeout_ms` as an additional alias (YAGNI — `timeout` covers the observed failure mode)
- Schema-level declaration of `timeout` as an alias
- Changes to `tool_timeout_secs` (project config) — unrelated
