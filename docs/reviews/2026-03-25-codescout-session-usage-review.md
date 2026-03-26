# Codescout Usage Review — Session Postmortem (2026-03-25)

## Scope

This report reviews how Codescout was used across the entire session while:

- reviewing the end-to-end ML pipeline in `eudis`
- reviewing `aegis-tracker`
- validating tests and environment assumptions
- moving between projects and writing follow-up documentation

The goal is not to judge the final review conclusions, but to assess the quality of the **tooling workflow** itself: where Codescout was used well, where it was misused, what patterns emerged, and what an improved operating model should look like.

---

## Executive Summary

The session produced useful technical findings, but the Codescout workflow was mixed.

Strong parts:

- project activation was handled correctly most of the time
- symbol bodies were used for the highest-value implementation reads
- project memories were used early to establish architecture and conventions
- tests were eventually rerun against the correct existing `.venv`
- the final tracker and pipeline reviews were grounded in real source, not guesses

Weak parts:

- `find_symbol` was underused once symbol names were already known
- `find_symbol` was sometimes called with regex-style patterns it does not support
- some searches used the wrong tool for the job, especially broad pattern matching where symbol navigation would have been cleaner
- buffer handling was occasionally clumsy, causing unnecessary detours into host file reads
- the workflow mixed Codescout tools and host search tools in ways that made the process noisier and less disciplined than necessary
- Python environment validation was initially done in the wrong order, producing avoidable false negatives

Net assessment:

- **Technical outcome:** solid
- **Codescout discipline:** inconsistent
- **Main failure mode:** not staying strict enough about tool semantics after initial exploration

---

## What Went Well

### 1. Project activation and restoration were handled deliberately

The session activated the `eudis` repo before doing any project-scoped work and later restored the active project back to `/home/marius`. When the user asked to save a report in another repository, the workflow switched into `code-explorer`, wrote the file there, and then restored again.

This is a strong pattern because Codescout is shared state. Forgetting to restore the active project is one of the easiest ways to silently break later tool calls.

**Good pattern observed:**

- activate target project before scoped work
- use project-scoped memories and symbol tools within that project
- restore the prior active project before finishing

### 2. Architecture and conventions were loaded before deep source review

The session started by reading Codescout memories such as:

- workspace `architecture`
- workspace `gotchas`
- workspace `language-patterns`
- per-project tracker and trainer architecture notes

This was the right opening move. It reduced blind searching and made later findings easier to interpret. In particular, the memory content clarified:

- the intended split between `aegis-detect` and tracker-side classification
- the expected model inputs and ONNX names
- the tracker’s role in the end-to-end system

**Good pattern observed:** memory-first orientation before code drilling.

### 3. Symbol bodies were used on the highest-value code paths

For the core implementation reads, the session did use `find_symbol(include_body=true)` effectively on important symbols such as:

- `generate_sample`
- `generate_dataset`
- `DroneClassifier`
- `FusionClassifier`
- `merge_raw_features`
- `TrajectoryPredictor`
- `MlTrajectoryPredictor/load`
- `MlTrajectoryPredictor/predict`
- `StoneSoupTracker/_fuse_detections`
- `StoneSoupTracker/_build_state_history`
- `StoneSoupTracker/process_cycle`
- `tracker.main.run`

This is where Codescout added the most value. These were the symbols that actually defined the pipeline contracts and tracker behavior.

**Good pattern observed:** use symbol-body reads for the code that actually implements behavior.

### 4. Tests were eventually validated against the correct repo environment

After the user flagged the venv issue, the workflow corrected itself by:

- checking the repo-local Python environment first
- confirming the workspace `.venv` already existed
- confirming installed packages through the Python environment tools
- rerunning `trainer` and `tracker` tests with the correct interpreter

This correction mattered because the initial failures were not code failures; they were environment-selection failures.

**Good pattern observed:** once challenged, the workflow switched from assumption to environment introspection.

### 5. Codescout was used well for targeted documentation output

For the final user request, Codescout was used appropriately to:

- activate the target repo
- inspect the documentation layout
- place the report in a reasonable existing docs area
- create the report in-repo instead of dropping an arbitrary file at the root

This is exactly the kind of documentation task Codescout handles cleanly.

---

## What Went Wrong

### 1. `find_symbol` was underused after symbol names were already known

Once the session had concrete symbol names, the workflow should have leaned much harder on `find_symbol` and `list_symbols`. Instead, it often shifted into pattern-search mode for things that were already symbol-shaped.

Examples of work that should have stayed mostly in symbol navigation:

- tracker cycle review
- trainer train/preprocess review
- export path review
- runtime predictor inspection

Instead, the session sometimes oscillated between symbol tools and raw pattern search.

**Why this matters:**

- symbol tools are more precise
- symbol tools reduce noise
- symbol tools make it easier to stay anchored to declarations and implementations
- symbol tools avoid fragile regex assumptions

### 2. `find_symbol` was misused with regex-style queries

At several points, `find_symbol` was called with patterns like:

- `preprocess_data|train|evaluate|main`
- `MlTrajectoryPredictor|NormalizationStats|predict|load|prepare|normalize`
- `generate_dataset|route_to_track_states|main`

Those are regex-like alternation patterns. `find_symbol` is not a regex engine. It looks up symbol names or name substrings. The empty results were not tool failures; they were invalid usage.

**Bad pattern observed:** treating `find_symbol` as if it were `search_pattern`.

**Correct replacement:**

- separate `find_symbol` calls for known symbol names
- or `list_symbols(path)` when the file is already known
- reserve `search_pattern` for actual text/regex search

### 3. Search scope was occasionally malformed or too broad

A few searches bundled multiple logical scopes into one path or used host search tools with mismatched assumptions about path filtering.

This showed up in forms like:

- trying to search multiple files/directories in one `path` argument
- mixing Codescout `search_pattern` with host `grep_search` when one tool would have sufficed
- using broader text search where a single-file symbol tree would have been cleaner

**Why this matters:**

- it produces empty or misleading results
- it adds avoidable tool churn
- it blurs whether a failure is from the codebase or the query shape

### 4. Buffer handling was inconsistent and sometimes awkward

Codescout returned large results through output buffers multiple times. Instead of staying inside Codescout’s buffered read flow consistently, the workflow sometimes fell back to host file reads of the generated JSON content, and once attempted to read a buffered result with the wrong handle shape.

This did not break the review, but it added friction and avoidable context switching.

**Bad pattern observed:** not committing to one buffer navigation path.

**Better pattern:**

- if Codescout returns `@tool_*` or `@file_*`, keep using Codescout buffer access primitives
- avoid switching to host file reads unless there is a specific need
- when a tool suggests `json_path`, prefer that first

### 5. The workflow mixed exploration modes too freely

The session had a recurring pattern:

1. use Codescout symbol tools
2. get a broad or truncated result
3. switch to text search
4. switch to host grep
5. return to Codescout symbol tools

That is workable, but it is not disciplined. The better pattern is:

1. use memory and `list_symbols` to orient
2. use `find_symbol(include_body=true)` for implementation
3. use `find_references` for impact
4. use `search_pattern` only for literals, regexes, or non-symbol data

**Main issue:** too much mode-switching, not enough narrowing.

### 6. Environment validation order was wrong initially

This was not a Codescout bug, but it affected the session strongly.

The workflow first configured Python in a way that surfaced the system interpreter, ran tests, and got failures like missing `torch` and missing tracker imports. Only after the user pushed back did the process verify that an existing project `.venv` was already available and properly provisioned.

This produced false-negative signals early in the review.

**Bad pattern observed:** running validation before confirming the intended project environment.

**Correct replacement:**

1. configure Python environment for the target project
2. inspect interpreter path and installed packages
3. only then run tests

### 7. Host tools were used as a crutch when Codescout would have been clearer

The session relied on host `grep_search`, `read_file`, and terminal execution in places where Codescout could have carried more of the load.

That was not always wrong. Sometimes host tools were genuinely faster for exact line anchors. But there were several moments where the host fallback came too early.

**Bad pattern observed:** switching out of Codescout because the previous Codescout query was poorly formed, rather than reformulating the Codescout query correctly.

---

## Good Patterns Encountered

This section extracts the best reusable patterns from the session.

### Pattern A: Memory-first architectural review

**Observed workflow:**

1. activate the project
2. read architecture and gotchas memories
3. read project-specific architecture memories
4. then inspect symbols

**Why it worked:**

- reduced redundant searching
- framed later findings in system context
- exposed intended contracts early

**Recommendation:** use this as the default opening for any Codescout-heavy review.

### Pattern B: Symbol-body reads for implementation truth

When the session needed to understand actual behavior, `find_symbol(include_body=true)` on named symbols was the strongest move.

**Why it worked:**

- returned precise implementations
- avoided summary-level ambiguity
- worked well across Python and Rust

**Recommendation:** once you know the symbol name, prefer this over regex.

### Pattern C: Use tests after source understanding, not before

The strongest validation happened after the source path was already understood. That allowed test results to confirm or qualify conclusions rather than drive blind debugging.

**Recommendation:** source first, tests second.

### Pattern D: Use Codescout for documentation placement

For the final report-writing task, the workflow sensibly inspected the target repository structure before creating the file.

**Recommendation:** for documentation tasks, use `list_dir` or `find_file` to place the output in an existing docs hierarchy.

---

## Bad Patterns Encountered

This section extracts the worst reusable patterns from the session.

### Anti-pattern A: Regex alternation inside `find_symbol`

Examples used in the session:

- `preprocess_data|train|evaluate|main`
- `MlTrajectoryPredictor|NormalizationStats|predict|load|prepare|normalize`

**Problem:** `find_symbol` is not regex search.

**Use instead:**

- separate `find_symbol` calls
- or `list_symbols(path)` first if the file is known

### Anti-pattern B: Broad search after already identifying the file

Once a file like `tracker/tracker/stonesoup_tracker.py` was known, there was little reason to keep doing broad text search for concepts that already corresponded to named methods.

**Problem:** wasted precision and increased noise.

**Use instead:** read the symbol tree, then the exact symbol bodies.

### Anti-pattern C: Switching tools because of malformed queries

Several apparent “tool misses” were actually query-shape mistakes. The correct response should have been to reformulate the Codescout query, not immediately jump to host tools.

### Anti-pattern D: Reading buffer outputs indirectly through host file plumbing

This added mental overhead and made the workflow more brittle.

### Anti-pattern E: Environment conclusions before interpreter confirmation

This produced avoidable false alarms.

---

## Concrete Mistakes From This Session

### Mistake 1: Treating `find_symbol` like regex search

This was the most obvious tool-semantic error.

**Impact:** empty results, extra recovery work, unnecessary fallback to other tools.

### Mistake 2: Not using `list_symbols(path)` aggressively enough once file paths were known

For example, after finding tracker and trainer files, the session could have spent more time in per-file symbol trees and less time in text search.

### Mistake 3: Mixed buffer navigation strategy

The session sometimes used Codescout’s buffer system correctly, but other times stepped outside it and re-read generated JSON files through host tooling.

### Mistake 4: Failing the “environment first” check on Python validation

The user had to intervene. That is a process error.

### Mistake 5: Letting search patterns substitute for impact analysis tools

There were places where `find_references` would have been the cleaner follow-up after understanding a symbol, especially for classification and tracker dataflow.

---

## Why These Failures Happened

### 1. The session started correctly, then drifted into expedient search

Once early context was gathered, the workflow became more opportunistic and less strict. That often happens when trying to move quickly through multiple code paths.

### 2. The distinction between tool families was not enforced tightly enough

Codescout has three very different classes of navigation:

- symbol-aware
- semantic
- regex/text

The session crossed between them too casually.

### 3. Some tool failures were misread as search failures rather than query-shape failures

This especially affected `find_symbol`.

### 4. There was pressure to produce line-anchored findings quickly

That led to earlier use of exact text search and host grep than ideal.

---

## What a Better Codescout Workflow Would Have Looked Like

### Phase 1: Orientation

1. `activate_project`
2. `memory(read, architecture/conventions/gotchas)`
3. `find_file` for relevant top-level paths
4. `list_symbols` for each high-value file

### Phase 2: Implementation drill-down

1. `find_symbol(name_path, include_body=true)` for core symbols
2. `find_references` for impact and callers
3. `hover` or `goto_definition` only where type clarification is needed

### Phase 3: Validation

1. confirm project environment first
2. run focused tests
3. compare test coverage against the risky branches just inspected

### Phase 4: Reporting

1. anchor findings to exact files/lines
2. distinguish code defects from test gaps and environment issues

That flow would have cut tool churn significantly.

---

## Recommended Operating Rules Going Forward

### Rule 1: Once a file is known, stop doing broad discovery in that area

Switch to `list_symbols(path)` and `find_symbol(include_body=true)`.

### Rule 2: Use `find_symbol` only for symbol lookup, never for regex alternation

If you need regex, use `search_pattern`.

### Rule 3: Use one search scope per `search_pattern` call

Do not cram multiple directories or pseudo-multi-path scopes into one path field.

### Rule 4: Stay inside Codescout buffers when possible

If a result comes back as a Codescout buffer, keep navigating it with Codescout.

### Rule 5: Validate environment before tests

Especially in Python repos.

### Rule 6: Prefer `find_references` after understanding a core symbol

Do not substitute regex search for actual reference analysis when the symbol is known.

### Rule 7: Use host grep only when the need is explicitly textual

Examples:

- exact literal anchor extraction
- line-number confirmation for reporting
- non-symbol strings such as ONNX tensor names or JSON keys

---

## Session Scorecard

### Codescout strengths shown in this session

- project activation and restoration: good
- memory-assisted orientation: good
- symbol-body extraction for core logic: good
- cross-project document creation: good

### Codescout weaknesses in this session

- disciplined symbol navigation: uneven
- query-shape correctness: uneven
- buffer handling consistency: uneven
- staying within Codescout instead of falling back early: weak
- environment validation order around testing: weak

### Overall rating

**B-**

Rationale:

- the session got to real technical findings
- the major architectural reads were correct
- the final outputs were useful
- but the tool usage was not clean enough to count as a strong exemplar of Codescout-first practice

---

## Final Assessment

The biggest lesson from this session is simple:

**Codescout worked best when it was used as intended, and most of the friction came from drifting away from its intended usage model rather than from inherent tool limitations.**

When the session used:

- project memories for orientation
- symbol bodies for implementation
- focused tests for validation

the workflow was strong.

When it used:

- regex habits in symbol tools
- broad text search after already knowing the file and symbol
- mixed buffer navigation paths
- premature environment conclusions

the workflow degraded.

The tool itself was not the main problem. The main problem was inconsistent adherence to tool semantics.

---

## Actionable Checklist For Future Sessions

Before deep review:

- activate the correct project
- read architecture/conventions memories
- confirm environment before running tests

When navigating code:

- use `list_symbols` once the file is known
- use `find_symbol(include_body=true)` once the symbol is known
- use `find_references` for impact
- use `search_pattern` only for true text/regex needs

When handling large results:

- stay inside Codescout buffer workflows
- avoid unnecessary host-file detours

When reporting:

- separate code defects from environment issues
- separate test gaps from implementation defects
- anchor findings to exact lines only after the code path is already understood
