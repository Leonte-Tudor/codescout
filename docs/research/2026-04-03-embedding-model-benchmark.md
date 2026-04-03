# Embedding Model Benchmark for Semantic Search

**Date:** 2026-04-03
**Status:** Active
**Purpose:** Reproducible quality comparison of embedding models for codescout's semantic search.

## Methodology

### Setup
1. Configure model in `.codescout/project.toml` under `[embeddings]`
2. Run `index_project(force: true)` — wait for completion
3. Run each test case via `semantic_search(query)` (default top-10)
4. Score each result set against the expected files

### Scoring

Each test case has **expected files** — the ground-truth files that a good search should surface.

**Per-query score (0-3):**
- **3** — All expected files appear in top 5
- **2** — All expected files appear in top 10, or majority in top 5
- **1** — At least one expected file in top 10
- **0** — No expected files in top 10

**Model score** = sum of all 20 query scores. Maximum = 60.

### What to record per model

| Field | Description |
|-------|-------------|
| Model | Full model string (e.g. `local:AllMiniLML6V2Q`) |
| Dimensions | Vector dimensionality |
| Index time | Wall-clock time for `index_project(force: true)` |
| Chunk count | From `index_status()` after indexing |
| DB size | `ls -lh .codescout/embeddings/project.db` |
| Total score | Sum of 20 query scores (max 60) |
| Per-query scores | Array of 20 individual scores |

---

## Test Cases

### Tier 1: Direct Concept (1-5)

Single concept, expects exact-match files. Tests basic retrieval.

#### TC-01: Exact type name
- **Query:** `RecoverableError`
- **Concepts:** Named type lookup
- **Expected files:**
  - `src/tools/mod.rs` (definition + impl)
  - `src/server.rs` (routing tests)
  - `docs/FEATURES.md` (documentation)

#### TC-02: Single feature area
- **Query:** `embedding model configuration`
- **Concepts:** Configuration subsystem
- **Expected files:**
  - `src/embed/mod.rs` (Embedder trait)
  - `docs/manual/src/configuration/embeddings.md`
  - `docs/manual/src/configuration/embedding-backends.md`

#### TC-03: Named module
- **Query:** `LSP client implementation`
- **Concepts:** Specific module
- **Expected files:**
  - `src/lsp/client.rs`
  - `src/lsp/ops.rs`
  - `src/lsp/manager.rs`

#### TC-04: Specific tool name
- **Query:** `run_command shell execution`
- **Concepts:** Tool by name
- **Expected files:**
  - `src/tools/command.rs`
  - `docs/manual/src/concepts/shell-integration.md`
  - `docs/manual/src/concepts/output-buffers.md`

#### TC-05: Data structure
- **Query:** `OutputGuard progressive disclosure capping`
- **Concepts:** Named pattern
- **Expected files:**
  - `src/tools/output.rs`
  - `docs/PROGRESSIVE_DISCOVERABILITY.md`

---

### Tier 2: Two-Concept Composition (6-12)

Requires understanding the relationship between two concepts.

#### TC-06: Feature + storage
- **Query:** `how are tool calls recorded in the usage database`
- **Concepts:** Usage tracking + SQLite schema
- **Expected files:**
  - `src/usage/db.rs`
  - `src/usage/mod.rs`
  - `docs/plans/2026-04-02-usage-traceability-design.md`

#### TC-07: Algorithm + domain
- **Query:** `section boundary detection in markdown editing`
- **Concepts:** Heading parsing + edit operations
- **Expected files:**
  - `src/tools/markdown.rs` (compute_section_end, perform_section_edit)
  - `src/tools/file_summary.rs` (parse_all_headings, heading_level)

#### TC-08: Migration + error
- **Query:** `dimension mismatch when switching embedding models`
- **Concepts:** Schema migration + model change
- **Expected files:**
  - `src/embed/index.rs` (build_index dimension check, maybe_migrate_to_vec0)
  - `src/embed/schema.rs`

#### TC-09: Security + feature
- **Query:** `dangerous command detection and safety checks`
- **Concepts:** Shell security + command validation
- **Expected files:**
  - `src/util/path_security.rs` (is_dangerous_command, check_tool_access)
  - `src/tools/command.rs`

#### TC-10: Pattern + overflow
- **Query:** `how overflow hints guide the agent to narrow results`
- **Concepts:** Progressive disclosure + agent guidance
- **Expected files:**
  - `src/tools/output.rs` (OutputGuard)
  - `docs/PROGRESSIVE_DISCOVERABILITY.md`
  - `src/prompts/server_instructions.md`

#### TC-11: Refactoring operation
- **Query:** `renaming a symbol across all references in the codebase`
- **Concepts:** LSP rename + file mutation
- **Expected files:**
  - `src/tools/symbol_edit.rs` (RenameSymbol)
  - `src/lsp/ops.rs`

#### TC-12: Configuration + resolution
- **Query:** `how the embedding URL and model prefix determine which backend is used`
- **Concepts:** Config resolution order + backend selection
- **Expected files:**
  - `src/embed/mod.rs` (backend resolution)
  - `docs/manual/src/configuration/embeddings.md` (resolution order section)

---

### Tier 3: Multi-Concept Cross-Cutting (13-17)

Three or more concepts; requires understanding architectural patterns.

#### TC-13: Crash + recovery + routing
- **Query:** `what happens when an LSP server crashes mid-request and how does the circuit breaker recover`
- **Concepts:** LSP lifecycle + error handling + resilience
- **Expected files:**
  - `src/lsp/client.rs`
  - `src/lsp/manager.rs`
  - `docs/manual/src/troubleshooting.md`

#### TC-14: Dispatch + error classification
- **Query:** `how does the tool dispatch pipeline handle both recoverable errors and fatal failures differently`
- **Concepts:** Tool trait + call_content + error routing
- **Expected files:**
  - `src/tools/mod.rs` (Tool trait, route_tool_error)
  - `src/server.rs` (dispatch + error tests)
  - `src/usage/mod.rs` (outcome classification)

#### TC-15: Force rebuild + migration
- **Query:** `end-to-end force re-indexing flow including dimension migration and vec0 table recreation`
- **Concepts:** Indexing pipeline + schema migration + vec0
- **Expected files:**
  - `src/embed/index.rs` (build_index, maybe_migrate_to_vec0)
  - `src/embed/mod.rs`

#### TC-16: Search pipeline
- **Query:** `how a semantic search query flows from input through embedding to KNN ranked results`
- **Concepts:** Embed → vec0 → search_scoped → ranking
- **Expected files:**
  - `src/tools/search.rs` (SemanticSearch tool)
  - `src/embed/index.rs` (search_scoped_vec0, search_multi_db)
  - `src/embed/mod.rs`

#### TC-17: Plugin integration
- **Query:** `how does the companion plugin route native Read and Grep calls to codescout MCP tools`
- **Concepts:** PreToolUse hooks + routing plugin + tool redirection
- **Expected files:**
  - `docs/manual/src/concepts/routing-plugin.md`
  - `docs/manual/src/getting-started/companion-plugin.md`

---

### Tier 4: Architectural Insight (18-20)

Requires understanding design decisions, consistency invariants, and cross-module patterns.

#### TC-18: Dual-path consistency
- **Query:** `why heading detection in parse_all_headings and compute_section_end must use the same code block tracking`
- **Concepts:** Two code paths that must agree + fenced block state
- **Expected files:**
  - `src/tools/markdown.rs` (compute_section_end)
  - `src/tools/file_summary.rs` (parse_all_headings)
  - `docs/TODO-tool-misbehaviors.md` (BUG-035)

#### TC-19: Activation wiring
- **Query:** `relationship between project activation, LSP server lifecycle, and tool context wiring`
- **Concepts:** Agent state + ActiveProject + ToolContext + LspManager
- **Expected files:**
  - `src/agent.rs` (Agent, ActiveProject)
  - `src/lsp/manager.rs` (LspManager)
  - `src/server.rs` (ToolContext construction)

#### TC-20: Prompt surface consistency
- **Query:** `how to keep the three prompt surfaces consistent when tools are renamed or behavior changes`
- **Concepts:** server_instructions.md + onboarding_prompt.md + build_system_prompt_draft
- **Expected files:**
  - `src/prompts/server_instructions.md`
  - `src/prompts/onboarding_prompt.md`
  - `src/tools/workflow.rs` (build_system_prompt_draft)

---

## Results

### Model: local:AllMiniLML6V2Q

| Field | Value |
|-------|-------|
| Dimensions | 384 |
| Context window | 256 tokens |
| Index time | ~15 seconds |
| Chunk count | 31,965 |
| DB size | *(to measure)* |
| **Total score** | **_/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | | |
| 02 | | |
| 03 | | |
| 04 | | |
| 05 | | |
| 06 | | |
| 07 | | |
| 08 | | |
| 09 | | |
| 10 | | |
| 11 | | |
| 12 | | |
| 13 | | |
| 14 | | |
| 15 | | |
| 16 | | |
| 17 | | |
| 18 | | |
| 19 | | |
| 20 | | |

---

### Model: nomic-embed-code (Q4_K_M, via llama.cpp on AMD GPU)

| Field | Value |
|-------|-------|
| Dimensions | 3584 |
| Context window | 32,768 tokens |
| Index time | ~25 minutes |
| Chunk count | 11,868 |
| DB size | *(to measure)* |
| **Total score** | **_/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | | |
| 02 | | |
| 03 | | |
| 04 | | |
| 05 | | |
| 06 | | |
| 07 | | |
| 08 | | |
| 09 | | |
| 10 | | |
| 11 | | |
| 12 | | |
| 13 | | |
| 14 | | |
| 15 | | |
| 16 | | |
| 17 | | |
| 18 | | |
| 19 | | |
| 20 | | |

---

### Model: *(template for additional models)*

| Field | Value |
|-------|-------|
| Dimensions | |
| Context window | |
| Index time | |
| Chunk count | |
| DB size | |
| **Total score** | **_/60** |

*(Copy the TC scoring table from above)*

---

## Notes

- **Chunk count varies by model** — models with larger context windows produce fewer, larger chunks.
  This affects retrieval: fewer chunks means each result covers more code, but may be less precise.
- **DB size scales with dimensions** — 3584-dim vectors use ~9x more storage than 384-dim.
- **Index time depends on hardware** — local ONNX models run on CPU; remote models depend on GPU/network.
- **Ground truth is subjective** — expected files are based on codescout's current architecture as of
  2026-04-03. Update them if the codebase changes significantly.
- **Run all 20 queries in the same session** to avoid MCP restart overhead between queries.
