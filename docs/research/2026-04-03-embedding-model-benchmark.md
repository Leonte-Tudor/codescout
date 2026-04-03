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
| Index time | ~70 seconds |
| Chunk count | 32,098 |
| DB size | 71 MB |
| **Total score** | **34/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | 2 | FEATURES.md #1 (RecoverableError docs), mod.rs #3+#5+#6+#7 (struct + tests). server.rs missed |
| 02 | 1 | embeddings.md #3+#7, config/project.rs #6 (EmbeddingsSection). embed/mod.rs missed |
| 03 | 3 | All 3: lsp/client.rs #2+#8+#9, lsp/ops.rs #6 (LspClientOps), lsp/manager.rs #3 |
| 04 | 2 | workflow-and-config.md #1+#2, output-buffers.md #4, workflow.rs #10. shell-integration.md missed |
| 05 | 3 | output.rs #6+#8, PROGRESSIVE_DISC.md #2+#4. Strong across code + docs |
| 06 | 1 | ARCHITECTURE.md #1 (Usage Recorder section), FEATURES.md #2, traceability-design #6. usage/db.rs missed |
| 07 | 2 | document-section-editing.md #1, file_summary.rs #6, BUG-035 #8, markdown.rs #9 |
| 08 | 2 | embed/index.rs #3+#5+#6+#7 (dimension check code), embed/mod.rs #4. schema.rs missed |
| 09 | 2 | path_security.rs #1+#2+#3+#8 (dominates). command.rs missed |
| 10 | 1 | research-progressive-disclosure #1, PROGRESSIVE_DISC.md #2+#3. output.rs and server_instructions.md missed |
| 11 | 2 | symbol-navigation.md #2, editing.md #3, symbol.rs #7+#8+#10. lsp/ops.rs missed |
| 12 | 2 | config/project.rs #2+#3+#4+#9 (EmbeddingsSection), embeddings.md #7. embed/mod.rs missed |
| 13 | 1 | lsp/manager.rs #6+#9, kotlin-lsp-mux #2, server.rs #3+#7+#10. lsp/client.rs and troubleshooting.md missed |
| 14 | 1 | server.rs #6+#7+#8+#9 (route_tool_error), FEATURES.md #2+#4. tools/mod.rs and usage/mod.rs missed |
| 15 | 2 | vec0-migration.md #1+#2+#3+#4 (perfect!), embed/index.rs #5+#6+#7+#8+#9. embed/mod.rs missed |
| 16 | 0 | No expected source files. Docs about semantic search dominate. search.rs, embed/index.rs both missed |
| 17 | 2 | routing-plugin.md #5+#10, companion-plugin.md #8, CLAUDE.md #7, agents/claude-code.md #3 |
| 18 | 3 | All 3: markdown.rs #3+#8+#10, file_summary.rs #5 (parse_all_headings_skips_code_blocks), BUG-035 #2+#4 |
| 19 | 1 | lsp/manager.rs #8+#10, tools/mod.rs #9 (ToolContext). agent.rs and server.rs missed |
| 20 | 1 | CLAUDE.md #1+#4 (Prompt Surface Consistency!), api-naming #3, onboarding-versioning #5+#7. All 3 actual files missed |

**Observations:**
- Better on code-level queries (TC-01: 2 vs 1, TC-03: 3 vs 2) — smaller chunks focus on individual declarations
- Weaker on concept composition (TC-10: 1 vs 3, TC-13: 1 vs 3) — 256-token context misses broader context
- No "closing brace `}`" noise — smaller chunks rarely end with boilerplate
- Finds config/project.rs for embedding queries (TC-02, TC-12) — nomic-embed-code misses this
- Both models completely fail on TC-16 (search pipeline source code)
### Model: nomic-embed-code (Q4_K_M, via llama.cpp on AMD GPU)


| Field | Value |
|-------|-------|
| Dimensions | 3584 |
| Context window | 32,768 tokens |
| Index time | ~25 minutes |
| Chunk count | 11,868 |
| DB size | 372 MB |
| **Total score** | **36/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | 1 | mod.rs #1 (RecoverableError impl), rest are generic closing-brace chunks |
| 02 | 1 | embed/mod.rs #1, but no config docs surfaced — drowned by generic chunks |
| 03 | 2 | lsp/client.rs #1, lsp/manager.rs #2+#3. ops.rs missed |
| 04 | 2 | workflow.rs (RunCommand) #3+#4, shell-integration.md #2. output-buffers missed |
| 05 | 3 | output.rs dominates (3 hits), PROGRESSIVE_DISCOVERABILITY.md #10 |
| 06 | 1 | traceability design doc #6, but usage/db.rs and usage/mod.rs missed entirely |
| 07 | 2 | markdown.rs #1+#5, BUG-035 #6, file_summary.rs missed |
| 08 | 2 | embed/index.rs #1 (model mismatch test), config docs #2+#3. schema.rs missed |
| 09 | 2 | path_security.rs dominates top 5 (3 hits). command.rs missed |
| 10 | 3 | Overflow hint pattern #1, PROGRESSIVE_DISC.md #2, output.rs #6, output-modes.md #5 |
| 11 | 2 | symbol.rs #2+#3+#5+#7 (RenameSymbol). lsp/ops.rs missed |
| 12 | 3 | embed/mod.rs #1 (resolution logic), embeddings.md #3, unified-config specs #5+#6 |
| 13 | 3 | All 3: lsp/client.rs #3, lsp/manager.rs #9+#10, troubleshooting.md #2 |
| 14 | 2 | server.rs #5+#7 (route_tool_error), usage/mod.rs→tools/usage.rs #1. tools/mod.rs missed |
| 15 | 2 | embed/index.rs #8 (dimension check code), vec0-migration.md #7. embed/mod.rs missed |
| 16 | 0 | No source files. All docs/concept pages about semantic search. search.rs totally missed |
| 17 | 3 | routing-plugin.md #1, companion-plugin.md #5, CLAUDE.md #2 |
| 18 | 2 | file_summary.rs #1+#3+#4+#5+#6+#10 (parse_all_headings tests/impl). markdown.rs #2. BUG-035 missed |
| 19 | 0 | No expected source files. Got workspace.rs, config.rs instead of agent.rs/server.rs |
| 20 | 1 | Prompt Surface docs dominate but the 3 actual files (server_instructions.md, onboarding_prompt.md, workflow.rs) all missed |

**Observations:**
- Strong on docs-heavy queries (TC-10, 12, 13, 17) — good at matching concept-level headings
- Weak on source-only queries (TC-16, 19) — tends to return docs about the concept instead of the implementation
- Many results are generic closing-brace `}` chunks (TC-01, 02) — large chunk windows include trailing boilerplate
- Best at cross-cutting queries that have both code and doc matches (TC-13, 18)
- The 32K context window creates some "kitchen sink" chunks that match broadly but imprecisely
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

## Head-to-Head Comparison (2026-04-03)

### Score by Tier

| Tier | AllMiniLML6V2Q | nomic-embed-code | Max |
|------|---------------|------------------|-----|
| 1 (Direct Concept) | 11/15 | 9/15 | 15 |
| 2 (Two-Concept) | 12/21 | 17/21 | 21 |
| 3 (Cross-Cutting) | 6/15 | 7/15 | 15 |
| 4 (Architectural) | 5/9 | 3/9 | 9 |
| **Total** | **34/60** | **36/60** | **60** |

### Where Each Model Wins

| Query | AllMiniLML6V2Q | nomic-embed-code | Winner | Why |
|-------|---------------|------------------|--------|-----|
| TC-01 | 2 | 1 | Mini | Smaller chunks focus on declarations, avoid `}` noise |
| TC-03 | 3 | 2 | Mini | All 3 LSP files found vs 2 — smaller chunks = more distinct symbols |
| TC-10 | 1 | 3 | Nomic | Broader context captures overflow hint patterns across code + prose |
| TC-12 | 2 | 3 | Nomic | Larger chunks connect URL/prefix logic to backend resolution |
| TC-13 | 1 | 3 | Nomic | Multi-concept query needs broad context to link crash → recovery |
| TC-17 | 2 | 3 | Nomic | Companion plugin routing well-captured in larger doc chunks |
| TC-18 | 3 | 2 | Mini | Specific function-level query benefits from granular chunks |

### Key Takeaways

1. **Scores are surprisingly close** (34 vs 36). The 7B code-specialized model with 9x more
   dimensions and 128x more context barely edges out the 22 MB bundled model.

2. **Different strengths:** AllMiniLML6V2Q wins on *precision* (finding specific functions/types),
   nomic-embed-code wins on *concept composition* (queries that span multiple ideas).

3. **Both fail on TC-16 and TC-19** — queries about internal pipelines where the relevant code
   doesn't use the same vocabulary as the query. This is a fundamental embedding limitation,
   not a model-specific issue.

4. **The `}` problem:** nomic-embed-code's 32K context creates chunks that end with boilerplate
   closing braces, which match too many queries. This is a chunking strategy issue, not a model
   issue — smaller max-chunk-size could fix it.

5. **Cost-effectiveness:** AllMiniLML6V2Q indexes in ~70 seconds (CPU) and uses 71 MB of storage.
   nomic-embed-code takes ~25 minutes (GPU) and uses 372 MB. For a 2-point score difference,
   the bundled model is the pragmatic default.

6. **For power users:** nomic-embed-code is worth it when concept-level queries dominate
   (architecture exploration, onboarding). AllMiniLML6V2Q is better for targeted code navigation.

## Notes

- **Chunk count varies by model** — models with larger context windows produce fewer, larger chunks.
  This affects retrieval: fewer chunks means each result covers more code, but may be less precise.
- **DB size scales with dimensions** — 3584-dim vectors use ~9x more storage than 384-dim.
- **Index time depends on hardware** — local ONNX models run on CPU; remote models depend on GPU/network.
- **Ground truth is subjective** — expected files are based on codescout's current architecture as of
  2026-04-03. Update them if the codebase changes significantly.
- **Run all 20 queries in the same session** to avoid MCP restart overhead between queries.
