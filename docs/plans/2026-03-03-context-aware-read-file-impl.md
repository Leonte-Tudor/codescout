# Context-Aware `read_file` Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enrich `read_file` with format-aware structural summaries (line ranges, schema shapes, heading trees) and navigation parameters (`heading`, `json_path`, `toml_key`) for Markdown, JSON, and TOML/YAML files.

**Architecture:** Extend `FileSummaryType` in `src/tools/file_summary.rs` to distinguish JSON/YAML/TOML (currently lumped as `Config`). Add rich summarizers with structural metadata. Add format-specific navigation params to `ReadFile`'s `input_schema` and `call()`. New extraction functions return section content + structural metadata. All navigation errors use `RecoverableError`.

**Tech Stack:** Rust, serde_json (existing), toml (existing), serde_yml (new dep for YAML parsing)

**Design doc:** `docs/plans/2026-03-03-context-aware-read-file-design.md`

---

## Phase 1: Enriched Markdown Summaries

### Task 1: Split `FileSummaryType::Config` and add `Json`/`Yaml`/`Toml` variants

**Files:**
- Modify: `src/tools/file_summary.rs` — `FileSummaryType` enum and `detect_file_type()`
- Modify: `src/tools/file.rs` — `ReadFile::call()` match arms

**Step 1: Write failing tests for new file type detection**

Add to the `tests` module in `src/tools/file_summary.rs`:

```rust
#[test]
fn detect_json_as_json() {
    assert!(matches!(detect_file_type("data.json"), FileSummaryType::Json));
    assert!(matches!(detect_file_type("package.json"), FileSummaryType::Json));
}

#[test]
fn detect_yaml_as_yaml() {
    assert!(matches!(detect_file_type("config.yaml"), FileSummaryType::Yaml));
    assert!(matches!(detect_file_type("docker-compose.yml"), FileSummaryType::Yaml));
}

#[test]
fn detect_toml_as_toml() {
    assert!(matches!(detect_file_type("Cargo.toml"), FileSummaryType::Toml));
    assert!(matches!(detect_file_type("pyproject.toml"), FileSummaryType::Toml));
}

#[test]
fn detect_other_config_still_works() {
    // .xml, .ini, .env, .lock, .cfg stay as Config
    assert!(matches!(detect_file_type("web.xml"), FileSummaryType::Config));
    assert!(matches!(detect_file_type(".env"), FileSummaryType::Config));
    assert!(matches!(detect_file_type("Cargo.lock"), FileSummaryType::Config));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL — `FileSummaryType::Json` etc. don't exist yet.

**Step 3: Implement the enum split**

In `src/tools/file_summary.rs`, replace the `FileSummaryType` enum:

```rust
pub enum FileSummaryType {
    Source,
    Markdown,
    Json,
    Yaml,
    Toml,
    Config, // remaining: .xml, .ini, .env, .lock, .cfg
    Generic,
}
```

Update `detect_file_type()`:

```rust
pub fn detect_file_type(path: &str) -> FileSummaryType {
    let lower = path.to_lowercase();
    const SOURCE_EXTS: &[&str] = &[
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".go", ".java", ".kt", ".kts", ".c", ".cpp",
        ".cc", ".cxx", ".h", ".swift", ".rb", ".cs", ".php", ".scala", ".ex", ".exs", ".hs",
        ".lua", ".sh", ".bash",
    ];
    const CONFIG_EXTS: &[&str] = &[".xml", ".ini", ".env", ".lock", ".cfg"];
    if SOURCE_EXTS.iter().any(|e| lower.ends_with(e)) {
        FileSummaryType::Source
    } else if lower.ends_with(".md") || lower.ends_with(".mdx") {
        FileSummaryType::Markdown
    } else if lower.ends_with(".json") {
        FileSummaryType::Json
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        FileSummaryType::Yaml
    } else if lower.ends_with(".toml") {
        FileSummaryType::Toml
    } else if CONFIG_EXTS.iter().any(|e| lower.ends_with(e)) {
        FileSummaryType::Config
    } else {
        FileSummaryType::Generic
    }
}
```

**Step 4: Update `ReadFile::call()` match arms**

In `src/tools/file.rs`, the buffering match statement dispatches per type. Update it to route `Json`, `Yaml`, `Toml` to their own summarizers (initially just delegate to `summarize_config` so nothing breaks):

```rust
// In ReadFile::call(), the match block:
crate::tools::file_summary::FileSummaryType::Json => {
    crate::tools::file_summary::summarize_config(&text)
}
crate::tools::file_summary::FileSummaryType::Yaml => {
    crate::tools::file_summary::summarize_config(&text)
}
crate::tools::file_summary::FileSummaryType::Toml => {
    crate::tools::file_summary::summarize_config(&text)
}
```

Also update `format_read_file_summary` in `file.rs` to handle new type strings `"json"`, `"yaml"`, `"toml"` (initially map them to the `"config"` rendering branch).

**Step 5: Fix the existing test `detect_toml_as_config`**

This test asserts that `.toml`, `.yaml`, `.json` are `Config`. Now `.json` → `Json`, `.yaml` → `Yaml`, `.toml` → `Toml`. Update or replace the test to match the new behavior (the new tests from Step 1 cover these cases).

**Step 6: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 7: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "refactor: split FileSummaryType::Config into Json/Yaml/Toml variants"
```

---

### Task 2: Enrich `summarize_markdown()` with line ranges and all heading levels

**Files:**
- Modify: `src/tools/file_summary.rs` — `summarize_markdown()`
- Modify: `src/tools/file.rs` — `format_read_file_summary()` for the `"markdown"` case

**Step 1: Write failing tests**

Add to `src/tools/file_summary.rs` tests module:

```rust
#[test]
fn markdown_summary_includes_line_ranges() {
    let content = "# Title\ntext\n## Section A\nmore text\nstill more\n## Section B\nfinal text";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 3);
    // # Title: lines 1-2 (before ## Section A)
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["line"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["end_line"].as_u64().unwrap(), 7); // H1 covers everything
    // ## Section A: lines 3-5
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section A");
    assert_eq!(headings[1]["line"].as_u64().unwrap(), 3);
    assert_eq!(headings[1]["end_line"].as_u64().unwrap(), 5);
    // ## Section B: lines 6-7
    assert_eq!(headings[2]["line"].as_u64().unwrap(), 6);
    assert_eq!(headings[2]["end_line"].as_u64().unwrap(), 7);
}

#[test]
fn markdown_summary_includes_h3_headings() {
    let content = "# Top\n## Mid\n### Deep\ntext\n## Other";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 4);
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Deep");
    assert_eq!(headings[2]["level"].as_u64().unwrap(), 3);
}

#[test]
fn markdown_summary_ignores_headings_in_code_blocks() {
    let content = "# Real\n```\n# Not a heading\n## Also not\n```\n## Real Too";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Real");
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Real Too");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL — current `summarize_markdown` returns flat strings, not objects with `line`/`end_line`.

**Step 3: Rewrite `summarize_markdown()`**

```rust
pub fn summarize_markdown(content: &str) -> Value {
    let line_count = content.lines().count();
    let mut in_code_block = false;

    // First pass: collect heading positions
    let mut raw_headings: Vec<(String, usize, usize)> = Vec::new(); // (text, level, line_1indexed)
    for (idx, line) in content.lines().enumerate() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if let Some(level) = heading_level(line) {
            raw_headings.push((line.to_string(), level, idx + 1));
        }
    }

    // Second pass: compute end_line for each heading.
    // A heading's section ends at the line before the next heading of same-or-higher level,
    // or at EOF. For parent headings, end_line extends to cover all children.
    let mut headings: Vec<Value> = Vec::new();
    for (i, (text, level, line)) in raw_headings.iter().enumerate() {
        // Find the next heading of same-or-higher level (lower level number = higher)
        let end_line = raw_headings[i + 1..]
            .iter()
            .find(|(_, l, _)| *l <= *level)
            .map(|(_, _, next_line)| next_line - 1)
            .unwrap_or(line_count);
        headings.push(serde_json::json!({
            "heading": text,
            "level": level,
            "line": line,
            "end_line": end_line,
        }));
    }

    // Cap at 30 headings to keep summaries compact
    headings.truncate(30);

    serde_json::json!({
        "type": "markdown",
        "line_count": line_count,
        "headings": headings,
    })
}

fn heading_level(line: &str) -> Option<usize> {
    if !line.starts_with('#') {
        return None;
    }
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    // Must have a space after hashes, and at most 6 levels
    if hashes >= 1 && hashes <= 6 && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(hashes)
    } else {
        None
    }
}
```

**Step 4: Update `format_read_file_summary()` markdown branch**

In `src/tools/file.rs`, the `"markdown"` case now receives objects instead of strings:

```rust
"markdown" => {
    if let Some(headings) = val["headings"].as_array() {
        if !headings.is_empty() {
            out.push_str("\n  Headings:");
            for h in headings {
                let heading = h["heading"].as_str().unwrap_or("?");
                let line = h["line"].as_u64().unwrap_or(0);
                let end_line = h["end_line"].as_u64().unwrap_or(0);
                let indent = "  ".repeat(
                    h["level"].as_u64().unwrap_or(1) as usize
                );
                out.push_str(&format!(
                    "\n    {indent}{heading}  L{line}-{end_line}"
                ));
            }
        }
    }
}
```

**Step 5: Update existing test `markdown_summary_extracts_h1_and_h2_only`**

This test asserts the old shape (flat strings, only H1/H2). Rename it and update assertions to match the new object format:

```rust
#[test]
fn markdown_summary_basic_structure() {
    let content = "# Title\nsome text\n## Section\nmore text\n### Sub\nnope";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    // Now includes H3
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section");
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Sub");
    assert_eq!(s["line_count"].as_u64().unwrap(), 6);
}
```

**Step 6: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 7: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: enrich markdown summaries with line ranges and all heading levels"
```

---

## Phase 2: JSON and TOML/YAML Summaries

### Task 3: Implement `summarize_json()` with schema shape

**Files:**
- Modify: `src/tools/file_summary.rs` — add `summarize_json()`
- Modify: `src/tools/file.rs` — wire `Json` variant to new summarizer + update `format_read_file_summary`

**Step 1: Write failing tests**

```rust
#[test]
fn json_summary_shows_top_level_keys() {
    let content = r#"{
  "name": "my-project",
  "version": "1.0.0",
  "dependencies": {
    "serde": "1.0",
    "tokio": "1.0"
  },
  "scripts": {
    "build": "cargo build"
  }
}"#;
    let s = summarize_json(content);
    assert_eq!(s["type"].as_str().unwrap(), "json");
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "object");
    let keys = schema["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0]["path"].as_str().unwrap(), "$.name");
    assert_eq!(keys[0]["type"].as_str().unwrap(), "string");
    assert_eq!(keys[2]["path"].as_str().unwrap(), "$.dependencies");
    assert_eq!(keys[2]["type"].as_str().unwrap(), "object");
    assert_eq!(keys[2]["count"].as_u64().unwrap(), 2);
}

#[test]
fn json_summary_handles_root_array() {
    let content = r#"[{"id": 1}, {"id": 2}, {"id": 3}]"#;
    let s = summarize_json(content);
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "array");
    assert_eq!(schema["count"].as_u64().unwrap(), 3);
    assert_eq!(schema["element_type"].as_str().unwrap(), "object");
}

#[test]
fn json_summary_handles_malformed_json() {
    let content = "{ not valid json !!";
    let s = summarize_json(content);
    // Falls back to generic summary
    assert_eq!(s["type"].as_str().unwrap(), "json");
    assert!(s["head"].is_string()); // generic fallback shape
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL — `summarize_json` doesn't exist.

**Step 3: Implement `summarize_json()`**

```rust
pub fn summarize_json(content: &str) -> Value {
    let line_count = content.lines().count();

    // Try to parse; on failure fall back to generic with json type label
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => {
            let mut fallback = summarize_generic_file(content);
            fallback["type"] = json!("json");
            return fallback;
        }
    };

    let schema = match &parsed {
        Value::Object(map) => {
            let keys: Vec<Value> = map
                .iter()
                .take(30) // cap top-level keys
                .map(|(k, v)| {
                    let mut entry = json!({
                        "path": format!("$.{}", k),
                        "type": json_type_name(v),
                    });
                    match v {
                        Value::Object(m) => { entry["count"] = json!(m.len()); }
                        Value::Array(a) => { entry["count"] = json!(a.len()); }
                        _ => {}
                    }
                    // Approximate line by searching for the key in content
                    if let Some(line) = find_json_key_line(content, k) {
                        entry["line"] = json!(line);
                    }
                    entry
                })
                .collect();
            json!({ "root_type": "object", "keys": keys })
        }
        Value::Array(arr) => {
            let element_type = arr.first()
                .map(json_type_name)
                .unwrap_or_else(|| "unknown".to_string());
            json!({
                "root_type": "array",
                "count": arr.len(),
                "element_type": element_type,
            })
        }
        other => json!({ "root_type": json_type_name(other) }),
    };

    json!({
        "type": "json",
        "line_count": line_count,
        "schema": schema,
    })
}

fn json_type_name(v: &Value) -> String {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_string()
}

/// Find the 1-indexed line number where a top-level JSON key first appears.
fn find_json_key_line(content: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\"", key);
    for (idx, line) in content.lines().enumerate() {
        if line.contains(&needle) {
            return Some((idx + 1) as u64);
        }
    }
    None
}
```

**Step 4: Wire into `ReadFile::call()` and `format_read_file_summary`**

In `file.rs`, change the `Json` match arm from `summarize_config` to `summarize_json`:

```rust
crate::tools::file_summary::FileSummaryType::Json => {
    crate::tools::file_summary::summarize_json(&text)
}
```

Add `"json"` branch to `format_read_file_summary`:

```rust
"json" => {
    if let Some(schema) = val.get("schema") {
        let root_type = schema["root_type"].as_str().unwrap_or("?");
        out.push_str(&format!("\n  Root: {root_type}"));
        if let Some(keys) = schema["keys"].as_array() {
            for k in keys {
                let path = k["path"].as_str().unwrap_or("?");
                let typ = k["type"].as_str().unwrap_or("?");
                let mut desc = format!("\n    {path}: {typ}");
                if let Some(count) = k["count"].as_u64() {
                    desc.push_str(&format!(" ({count} items)"));
                }
                if let Some(line) = k["line"].as_u64() {
                    desc.push_str(&format!("  L{line}"));
                }
                out.push_str(&desc);
            }
        }
        if let Some(count) = schema["count"].as_u64() {
            out.push_str(&format!("\n    Count: {count}"));
            if let Some(elem) = schema["element_type"].as_str() {
                out.push_str(&format!(" (element type: {elem})"));
            }
        }
    }
}
```

**Step 5: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add JSON schema-shape summary for read_file"
```

---

### Task 4: Implement `summarize_toml()` with table structure

**Files:**
- Modify: `src/tools/file_summary.rs` — add `summarize_toml()`
- Modify: `src/tools/file.rs` — wire `Toml` variant + update formatter

**Step 1: Write failing tests**

```rust
#[test]
fn toml_summary_shows_tables() {
    let content = "[package]\nname = \"foo\"\nversion = \"1.0\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"\n\n[dev-dependencies]\ntempfile = \"3\"";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    assert_eq!(s["format"].as_str().unwrap(), "toml");
    let sections = s["sections"].as_array().unwrap();
    assert!(sections.len() >= 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "[package]");
    assert!(sections[0]["line"].as_u64().unwrap() >= 1);
    assert!(sections[0]["end_line"].as_u64().is_some());
}

#[test]
fn toml_summary_handles_nested_tables() {
    let content = "[package]\nname = \"foo\"\n\n[profile.release]\nopt-level = 3\nlto = true";
    let s = summarize_toml(content);
    let sections = s["sections"].as_array().unwrap();
    assert!(sections.iter().any(|s| s["key"].as_str().unwrap() == "[profile.release]"));
}

#[test]
fn toml_summary_handles_malformed() {
    let content = "not valid toml [[[";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    // Falls back, but still has line_count
    assert!(s["line_count"].as_u64().is_some());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL

**Step 3: Implement `summarize_toml()`**

Use a line-scanning approach to find `[table]` headers (no need to parse the TOML value tree for the summary — just find section boundaries):

```rust
pub fn summarize_toml(content: &str) -> Value {
    let line_count = content.lines().count();

    // Scan for TOML table headers: [name] or [[name]]
    let mut sections: Vec<(String, usize)> = Vec::new(); // (header, line_1indexed)
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if (trimmed.starts_with('[') && !trimmed.starts_with("[#"))
            && trimmed.ends_with(']')
        {
            sections.push((trimmed.to_string(), idx + 1));
        }
    }

    // If no table headers found, try parsing as TOML and list top-level keys
    if sections.is_empty() {
        if let Ok(table) = content.parse::<toml::Table>() {
            let keys: Vec<Value> = table.keys()
                .take(20)
                .map(|k| {
                    let line = find_toml_key_line(content, k);
                    json!({ "key": k, "line": line.unwrap_or(0) })
                })
                .collect();
            return json!({
                "type": "toml",
                "format": "toml",
                "line_count": line_count,
                "keys": keys,
            });
        }
        // Malformed: fall back to generic with label
        let mut fallback = summarize_generic_file(content);
        fallback["type"] = json!("toml");
        fallback["format"] = json!("toml");
        return fallback;
    }

    // Compute end_line for each section
    let mut result_sections: Vec<Value> = Vec::new();
    for (i, (header, line)) in sections.iter().enumerate() {
        let end_line = sections
            .get(i + 1)
            .map(|(_, next)| next - 1)
            .unwrap_or(line_count);
        result_sections.push(json!({
            "key": header,
            "line": line,
            "end_line": end_line,
        }));
    }
    result_sections.truncate(30);

    json!({
        "type": "toml",
        "format": "toml",
        "line_count": line_count,
        "sections": result_sections,
    })
}

fn find_toml_key_line(content: &str, key: &str) -> Option<u64> {
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed[key.len()..].trim_start().starts_with('=') {
            return Some((idx + 1) as u64);
        }
    }
    None
}
```

**Step 4: Wire into `ReadFile::call()` and `format_read_file_summary`**

```rust
// In ReadFile::call():
crate::tools::file_summary::FileSummaryType::Toml => {
    crate::tools::file_summary::summarize_toml(&text)
}
```

Add `"toml"` branch to `format_read_file_summary`:

```rust
"toml" => {
    if let Some(sections) = val["sections"].as_array() {
        out.push_str("\n  Sections:");
        for s in sections {
            let key = s["key"].as_str().unwrap_or("?");
            let line = s["line"].as_u64().unwrap_or(0);
            let end = s["end_line"].as_u64().unwrap_or(0);
            out.push_str(&format!("\n    {key}  L{line}-{end}"));
        }
    }
    if let Some(keys) = val["keys"].as_array() {
        out.push_str("\n  Keys:");
        for k in keys {
            let key = k["key"].as_str().unwrap_or("?");
            let line = k["line"].as_u64().unwrap_or(0);
            out.push_str(&format!("\n    {key}  L{line}"));
        }
    }
}
```

**Step 5: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add TOML table-structure summary for read_file"
```

---

### Task 5: Implement `summarize_yaml()` with section structure

**Files:**
- Modify: `Cargo.toml` — add `serde_yml` dependency
- Modify: `src/tools/file_summary.rs` — add `summarize_yaml()`
- Modify: `src/tools/file.rs` — wire `Yaml` variant + update formatter

**Step 1: Add dependency**

Add to `Cargo.toml` under `[dependencies]`:

```toml
serde_yml = "0.0.12"
```

Run: `cargo check` to verify the dependency resolves.

**Step 2: Write failing tests**

```rust
#[test]
fn yaml_summary_shows_top_level_keys() {
    let content = "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080\nlogging:\n  level: debug";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "database");
    assert!(sections[0]["line"].as_u64().is_some());
}

#[test]
fn yaml_summary_handles_malformed() {
    let content = "not:\n  valid:\nyaml: [unclosed";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    assert!(s["line_count"].as_u64().is_some());
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL

**Step 4: Implement `summarize_yaml()`**

YAML doesn't have explicit section headers like TOML, so scan for top-level keys (lines that start with a non-space character followed by a colon):

```rust
pub fn summarize_yaml(content: &str) -> Value {
    let line_count = content.lines().count();

    // Scan for top-level keys: lines starting with a non-whitespace char, containing ':'
    let mut sections: Vec<(String, usize)> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() || trimmed == "---" || trimmed == "..." {
            continue;
        }
        // Top-level key: starts at column 0, has a colon
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(colon_pos) = trimmed.find(':') {
                let key = trimmed[..colon_pos].trim().to_string();
                if !key.is_empty() && !key.starts_with('-') {
                    sections.push((key, idx + 1));
                }
            }
        }
    }

    if sections.is_empty() {
        let mut fallback = summarize_generic_file(content);
        fallback["type"] = json!("yaml");
        fallback["format"] = json!("yaml");
        return fallback;
    }

    // Compute end_line for each section
    let mut result_sections: Vec<Value> = Vec::new();
    for (i, (key, line)) in sections.iter().enumerate() {
        let end_line = sections
            .get(i + 1)
            .map(|(_, next)| next - 1)
            .unwrap_or(line_count);
        result_sections.push(json!({
            "key": key,
            "line": line,
            "end_line": end_line,
        }));
    }
    result_sections.truncate(30);

    json!({
        "type": "yaml",
        "format": "yaml",
        "line_count": line_count,
        "sections": result_sections,
    })
}
```

Note: `serde_yml` is added as a dependency but this initial implementation uses line-scanning (same as TOML) since we just need structural boundaries for the summary. The parsed-YAML dependency will be needed in Phase 3 for `toml_key` navigation on YAML files.

**Step 5: Wire into `ReadFile::call()` and `format_read_file_summary`**

```rust
// In ReadFile::call():
crate::tools::file_summary::FileSummaryType::Yaml => {
    crate::tools::file_summary::summarize_yaml(&text)
}
```

Add `"yaml"` to `format_read_file_summary` — same rendering as `"toml"` sections:

```rust
"yaml" | "toml" => {
    // ... same section rendering logic for both
}
```

**Step 6: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add YAML section-structure summary for read_file"
```

---

## Phase 3: Navigation Parameters

### Task 6: Add `heading` parameter for Markdown navigation

**Files:**
- Modify: `src/tools/file_summary.rs` — add `extract_markdown_section()`
- Modify: `src/tools/file.rs` — add `heading` to `input_schema()`, handle in `call()`

**Step 1: Write failing tests for section extraction**

Add to `src/tools/file_summary.rs`:

```rust
#[test]
fn extract_markdown_section_exact_match() {
    let content = "# Intro\nwelcome\n## Setup\ndo this\nand that\n## Usage\nuse it";
    let result = extract_markdown_section(content, "## Setup").unwrap();
    assert_eq!(result.content, "## Setup\ndo this\nand that");
    assert_eq!(result.line_range, (3, 5));
    assert_eq!(result.breadcrumb, vec!["# Intro", "## Setup"]);
    assert_eq!(result.siblings, vec!["## Usage"]);
}

#[test]
fn extract_markdown_section_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails";
    let result = extract_markdown_section(content, "## Auth").unwrap();
    assert!(result.content.contains("Authentication Guide"));
}

#[test]
fn extract_markdown_section_not_found() {
    let content = "# Title\n## Setup\ntext";
    let result = extract_markdown_section(content, "## Nonexistent");
    assert!(result.is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_summary::tests -- --nocapture`
Expected: FAIL

**Step 3: Implement `extract_markdown_section()`**

```rust
pub struct SectionResult {
    pub content: String,
    pub line_range: (usize, usize), // 1-indexed, inclusive
    pub breadcrumb: Vec<String>,
    pub siblings: Vec<String>,
    pub format: String,
}

pub fn extract_markdown_section(content: &str, heading_query: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_markdown(content);
    let headings = summary["headings"].as_array()
        .ok_or_else(|| RecoverableError::new("no headings found in file"))?;

    // Find matching heading: exact first, then prefix, then substring
    let query_lower = heading_query.to_lowercase();
    let matched_idx = headings.iter().position(|h| {
        h["heading"].as_str().unwrap_or("") == heading_query
    }).or_else(|| headings.iter().position(|h| {
        h["heading"].as_str().unwrap_or("").to_lowercase().starts_with(&query_lower)
    })).or_else(|| headings.iter().position(|h| {
        h["heading"].as_str().unwrap_or("").to_lowercase().contains(&query_lower)
    }));

    let idx = matched_idx.ok_or_else(|| {
        let available: Vec<&str> = headings.iter()
            .filter_map(|h| h["heading"].as_str())
            .take(15)
            .collect();
        RecoverableError::with_hint(
            format!("heading '{}' not found", heading_query),
            format!("Available headings: {}", available.join(", ")),
        )
    })?;

    let matched = &headings[idx];
    let line = matched["line"].as_u64().unwrap_or(1) as usize;
    let end_line = matched["end_line"].as_u64().unwrap_or(1) as usize;
    let level = matched["level"].as_u64().unwrap_or(1) as usize;

    // Extract content
    let lines: Vec<&str> = content.lines().collect();
    let section_content = lines[line - 1..end_line].join("\n");

    // Build breadcrumb: walk backwards from matched heading, collecting parents
    let mut breadcrumb = Vec::new();
    let mut current_level = level;
    for i in (0..=idx).rev() {
        let h_level = headings[i]["level"].as_u64().unwrap_or(1) as usize;
        if h_level < current_level || i == idx {
            breadcrumb.push(headings[i]["heading"].as_str().unwrap_or("?").to_string());
            current_level = h_level;
        }
    }
    breadcrumb.reverse();

    // Find siblings: same level, same parent
    let parent_level = if level > 1 { level - 1 } else { 0 };
    let mut siblings = Vec::new();
    for h in headings {
        let h_level = h["level"].as_u64().unwrap_or(0) as usize;
        let h_text = h["heading"].as_str().unwrap_or("");
        if h_level == level && h_text != matched["heading"].as_str().unwrap_or("") {
            siblings.push(h_text.to_string());
        }
    }

    Ok(SectionResult {
        content: section_content,
        line_range: (line, end_line),
        breadcrumb,
        siblings,
        format: "markdown".to_string(),
    })
}
```

**Step 4: Add `heading` parameter to `ReadFile`**

In `src/tools/file.rs`, update `input_schema()`:

```rust
"heading": { "type": "string", "description": "Extract a Markdown section by heading text (e.g. \"## Authentication\"). Returns section content with structural metadata. Mutually exclusive with start_line/end_line and other navigation params." },
```

In `call()`, after the path resolution and before the `start_line`/`end_line` check, add:

```rust
let heading = input["heading"].as_str();
let json_path = input["json_path"].as_str();
let toml_key = input["toml_key"].as_str();
let nav_param_count = [heading.is_some(), json_path.is_some(), toml_key.is_some()]
    .iter().filter(|&&x| x).count();

if nav_param_count > 1 {
    return Err(RecoverableError::with_hint(
        "only one navigation parameter allowed at a time",
        "Use heading OR json_path OR toml_key, not multiple",
    ).into());
}

if nav_param_count > 0 && (start_line.is_some() || end_line.is_some()) {
    return Err(RecoverableError::with_hint(
        "navigation parameters are mutually exclusive with start_line/end_line",
        "Use either heading/json_path/toml_key OR start_line+end_line",
    ).into());
}

// Handle heading navigation
if let Some(heading_query) = heading {
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    if !matches!(file_type, crate::tools::file_summary::FileSummaryType::Markdown) {
        return Err(RecoverableError::with_hint(
            "heading parameter is only supported for Markdown files",
            "For JSON files use json_path, for TOML/YAML use toml_key",
        ).into());
    }
    let result = crate::tools::file_summary::extract_markdown_section(&text, heading_query)?;
    // If extracted section is itself large, buffer it
    if result.content.lines().count() > crate::tools::file_summary::FILE_BUFFER_THRESHOLD {
        let file_id = ctx.output_buffer.store_file(
            resolved.to_string_lossy().to_string(),
            result.content.clone(),
        );
        return Ok(json!({
            "line_range": [result.line_range.0, result.line_range.1],
            "breadcrumb": result.breadcrumb,
            "siblings": result.siblings,
            "format": "markdown",
            "file_id": file_id,
            "hint": format!("Section content stored as {}. Query with: run_command(\"grep/sed {}\")", file_id, file_id),
        }));
    }
    return Ok(json!({
        "content": result.content,
        "line_range": [result.line_range.0, result.line_range.1],
        "breadcrumb": result.breadcrumb,
        "siblings": result.siblings,
        "format": "markdown",
    }));
}
```

**Step 5: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add heading navigation parameter for Markdown files"
```

---

### Task 7: Add `json_path` parameter for JSON navigation

**Files:**
- Modify: `src/tools/file_summary.rs` — add `extract_json_path()`
- Modify: `src/tools/file.rs` — add `json_path` to `input_schema()`, handle in `call()`

**Step 1: Write failing tests**

```rust
#[test]
fn extract_json_path_top_level_key() {
    let content = r#"{"name": "test", "deps": {"a": 1, "b": 2}}"#;
    let result = extract_json_path(content, "$.deps").unwrap();
    assert!(result.content.contains("\"a\""));
    assert!(result.content.contains("\"b\""));
    assert_eq!(result.format, "json");
}

#[test]
fn extract_json_path_nested() {
    let content = r#"{"db": {"connection": {"host": "localhost", "port": 5432}}}"#;
    let result = extract_json_path(content, "$.db.connection").unwrap();
    assert!(result.content.contains("localhost"));
}

#[test]
fn extract_json_path_array_index() {
    let content = r#"{"users": [{"name": "alice"}, {"name": "bob"}]}"#;
    let result = extract_json_path(content, "$.users[0]").unwrap();
    assert!(result.content.contains("alice"));
    assert!(!result.content.contains("bob"));
}

#[test]
fn extract_json_path_not_found() {
    let content = r#"{"name": "test"}"#;
    let result = extract_json_path(content, "$.nonexistent");
    assert!(result.is_err());
}
```

**Step 2: Implement `extract_json_path()`**

Parse the path into segments (split on `.`, handle `[N]`), walk the JSON tree:

```rust
pub fn extract_json_path(content: &str, path: &str) -> Result<SectionResult, RecoverableError> {
    let parsed: Value = serde_json::from_str(content).map_err(|e| {
        RecoverableError::with_hint(
            format!("failed to parse JSON: {}", e),
            "Ensure the file contains valid JSON",
        )
    })?;

    // Parse path: "$.key1.key2[0].key3" -> ["key1", "key2", "[0]", "key3"]
    let segments = parse_json_path(path)?;

    // Walk the tree
    let mut current = &parsed;
    for seg in &segments {
        current = resolve_json_segment(current, seg).ok_or_else(|| {
            let available = match current {
                Value::Object(m) => m.keys().take(10).cloned().collect::<Vec<_>>().join(", "),
                Value::Array(a) => format!("array with {} elements (0..{})", a.len(), a.len().saturating_sub(1)),
                _ => format!("{} (not navigable)", json_type_name(current)),
            };
            RecoverableError::with_hint(
                format!("path segment '{}' not found at {}", seg, path),
                format!("Available: {}", available),
            )
        })?;
    }

    let pretty = serde_json::to_string_pretty(current).unwrap_or_else(|_| current.to_string());
    let type_name = json_type_name(current);
    let count = match current {
        Value::Object(m) => Some(m.len()),
        Value::Array(a) => Some(a.len()),
        _ => None,
    };

    // Approximate line range by finding the key in content
    let last_key = segments.last().map(|s| s.as_str()).unwrap_or("");
    let start_line = find_json_key_line(content, last_key).unwrap_or(1) as usize;
    // Rough end_line based on pretty-printed size
    let end_line = start_line + pretty.lines().count();

    let mut result = SectionResult {
        content: pretty,
        line_range: (start_line, end_line),
        breadcrumb: Vec::new(),
        siblings: Vec::new(),
        format: "json".to_string(),
    };

    // Add type/count as extra fields will be added to the JSON response in call()

    Ok(result)
}

fn parse_json_path(path: &str) -> Result<Vec<String>, RecoverableError> {
    let path = path.strip_prefix("$.").or_else(|| path.strip_prefix("$")).unwrap_or(path);
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let mut segments = Vec::new();
    for part in path.split('.') {
        // Handle array index: "key[0]" -> "key", "[0]"
        if let Some(bracket_pos) = part.find('[') {
            let key = &part[..bracket_pos];
            if !key.is_empty() {
                segments.push(key.to_string());
            }
            // Extract index
            let idx_str = &part[bracket_pos..];
            segments.push(idx_str.to_string());
        } else {
            segments.push(part.to_string());
        }
    }
    Ok(segments)
}

fn resolve_json_segment<'a>(value: &'a Value, segment: &str) -> Option<&'a Value> {
    if segment.starts_with('[') && segment.ends_with(']') {
        let idx: usize = segment[1..segment.len() - 1].parse().ok()?;
        value.as_array()?.get(idx)
    } else {
        value.get(segment)
    }
}
```

**Step 3: Wire `json_path` into `ReadFile::call()`**

Add to `input_schema()`:

```rust
"json_path": { "type": "string", "description": "Extract a JSON subtree by path (e.g. \"$.dependencies\", \"$.users[0]\"). Returns pretty-printed content with type info. Mutually exclusive with start_line/end_line and other navigation params." },
```

Add handler in `call()` (after the heading handler):

```rust
if let Some(jp) = json_path {
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    if !matches!(file_type, crate::tools::file_summary::FileSummaryType::Json) {
        return Err(RecoverableError::with_hint(
            "json_path parameter is only supported for JSON files",
            "For Markdown files use heading, for TOML/YAML use toml_key",
        ).into());
    }
    let result = crate::tools::file_summary::extract_json_path(&text, jp)?;
    return Ok(json!({
        "content": result.content,
        "line_range": [result.line_range.0, result.line_range.1],
        "path": jp,
        "format": "json",
    }));
}
```

**Step 4: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add json_path navigation parameter for JSON files"
```

---

### Task 8: Add `toml_key` parameter for TOML/YAML navigation

**Files:**
- Modify: `src/tools/file_summary.rs` — add `extract_toml_key()`, `extract_yaml_key()`
- Modify: `src/tools/file.rs` — add `toml_key` to `input_schema()`, handle in `call()`

**Step 1: Write failing tests**

```rust
#[test]
fn extract_toml_key_table() {
    let content = "[package]\nname = \"foo\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"";
    let result = extract_toml_key(content, "dependencies").unwrap();
    assert!(result.content.contains("serde"));
    assert!(result.content.contains("tokio"));
    assert_eq!(result.format, "toml");
}

#[test]
fn extract_toml_key_nested() {
    let content = "[package]\nname = \"foo\"\n\n[profile.release]\nopt-level = 3";
    let result = extract_toml_key(content, "profile.release").unwrap();
    assert!(result.content.contains("opt-level"));
}

#[test]
fn extract_toml_key_not_found() {
    let content = "[package]\nname = \"foo\"";
    let result = extract_toml_key(content, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_yaml_key_section() {
    let content = "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080";
    let result = extract_yaml_key(content, "database").unwrap();
    assert!(result.content.contains("host"));
    assert!(result.content.contains("localhost"));
    assert_eq!(result.format, "yaml");
}
```

**Step 2: Implement `extract_toml_key()` and `extract_yaml_key()`**

For TOML, use the summary's section data to find the section, then extract lines:

```rust
pub fn extract_toml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_toml(content);

    // Check sections first (table headers)
    if let Some(sections) = summary["sections"].as_array() {
        let table_name = if key.starts_with('[') { key.to_string() } else { format!("[{}]", key) };
        if let Some(matched) = sections.iter().find(|s| {
            let k = s["key"].as_str().unwrap_or("");
            k == table_name || k == format!("[{}]", key) || k == format!("[[{}]]", key)
        }) {
            let line = matched["line"].as_u64().unwrap_or(1) as usize;
            let end_line = matched["end_line"].as_u64().unwrap_or(1) as usize;
            let lines: Vec<&str> = content.lines().collect();
            let section_content = lines[line - 1..end_line].join("\n");
            let siblings: Vec<String> = sections.iter()
                .filter_map(|s| s["key"].as_str())
                .filter(|k| *k != matched["key"].as_str().unwrap_or(""))
                .map(|s| s.to_string())
                .collect();
            return Ok(SectionResult {
                content: section_content,
                line_range: (line, end_line),
                breadcrumb: vec![matched["key"].as_str().unwrap_or("?").to_string()],
                siblings,
                format: "toml".to_string(),
            });
        }
    }

    // Try parsing and extracting by dot-path
    let table: toml::Table = content.parse().map_err(|e| {
        RecoverableError::with_hint(
            format!("failed to parse TOML: {}", e),
            "Ensure the file contains valid TOML",
        )
    })?;

    let segments: Vec<&str> = key.split('.').collect();
    let mut current: &toml::Value = &toml::Value::Table(table.clone());
    for seg in &segments {
        current = current.get(seg).ok_or_else(|| {
            let available = match current {
                toml::Value::Table(t) => t.keys().take(10).cloned().collect::<Vec<_>>().join(", "),
                _ => "not a table".to_string(),
            };
            RecoverableError::with_hint(
                format!("key '{}' not found in TOML", key),
                format!("Available keys: {}", available),
            )
        })?;
    }

    let pretty = toml::to_string_pretty(current).unwrap_or_else(|_| format!("{:?}", current));
    Ok(SectionResult {
        content: pretty,
        line_range: (1, content.lines().count()), // approximate
        breadcrumb: segments.iter().map(|s| s.to_string()).collect(),
        siblings: Vec::new(),
        format: "toml".to_string(),
    })
}

pub fn extract_yaml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_yaml(content);

    // Use section boundaries from summary
    if let Some(sections) = summary["sections"].as_array() {
        if let Some(matched) = sections.iter().find(|s| {
            s["key"].as_str().unwrap_or("") == key
        }) {
            let line = matched["line"].as_u64().unwrap_or(1) as usize;
            let end_line = matched["end_line"].as_u64().unwrap_or(1) as usize;
            let lines: Vec<&str> = content.lines().collect();
            let section_content = lines[line - 1..end_line].join("\n");
            let siblings: Vec<String> = sections.iter()
                .filter_map(|s| s["key"].as_str())
                .filter(|k| *k != key)
                .map(|s| s.to_string())
                .collect();
            return Ok(SectionResult {
                content: section_content,
                line_range: (line, end_line),
                breadcrumb: vec![key.to_string()],
                siblings,
                format: "yaml".to_string(),
            });
        }
    }

    // Not found
    let available: Vec<String> = summary["sections"].as_array()
        .map(|arr| arr.iter().filter_map(|s| s["key"].as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    Err(RecoverableError::with_hint(
        format!("key '{}' not found in YAML", key),
        format!("Available keys: {}", available.join(", ")),
    ))
}
```

**Step 3: Wire `toml_key` into `ReadFile`**

Add to `input_schema()`:

```rust
"toml_key": { "type": "string", "description": "Extract a TOML table or YAML section by key (e.g. \"dependencies\", \"database.connection\"). Returns section content with structural metadata. Mutually exclusive with start_line/end_line and other navigation params." },
```

Add handler in `call()`:

```rust
if let Some(tk) = toml_key {
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    match file_type {
        crate::tools::file_summary::FileSummaryType::Toml => {
            let result = crate::tools::file_summary::extract_toml_key(&text, tk)?;
            return Ok(json!({
                "content": result.content,
                "line_range": [result.line_range.0, result.line_range.1],
                "breadcrumb": result.breadcrumb,
                "siblings": result.siblings,
                "format": "toml",
            }));
        }
        crate::tools::file_summary::FileSummaryType::Yaml => {
            let result = crate::tools::file_summary::extract_yaml_key(&text, tk)?;
            return Ok(json!({
                "content": result.content,
                "line_range": [result.line_range.0, result.line_range.1],
                "breadcrumb": result.breadcrumb,
                "siblings": result.siblings,
                "format": "yaml",
            }));
        }
        _ => {
            return Err(RecoverableError::with_hint(
                "toml_key parameter is only supported for TOML and YAML files",
                "For Markdown files use heading, for JSON use json_path",
            ).into());
        }
    }
}
```

**Step 4: Run all tests**

Run: `cargo test --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/tools/file_summary.rs src/tools/file.rs
git commit -m "feat: add toml_key navigation parameter for TOML/YAML files"
```

---

## Phase 4: Documentation and Polish

### Task 9: Update server instructions and tool description

**Files:**
- Modify: `src/prompts/server_instructions.md` — document new params and summary formats
- Modify: `src/tools/file.rs` — update `ReadFile::description()` to mention navigation params

**Step 1: Update `ReadFile::description()`**

```rust
fn description(&self) -> &str {
    "Read the contents of a file. Optionally restrict to a line range. Large files (>200 lines) are automatically buffered and returned as a summary + @file_* handle. Use start_line + end_line to read a specific range directly. For symbol-level navigation of source code, prefer symbol tools. Format-aware navigation: use heading for Markdown sections, json_path for JSON subtrees, toml_key for TOML tables or YAML sections."
}
```

**Step 2: Update `server_instructions.md`**

In the `read_file` description section, add:

```markdown
- `heading` — (Markdown only) Extract section by heading text. Returns content + line range + breadcrumb + siblings.
- `json_path` — (JSON only) Extract subtree by path (e.g. `$.dependencies`, `$.users[0]`).
- `toml_key` — (TOML/YAML only) Extract table or section by key (e.g. `dependencies`, `database.connection`).
```

**Step 3: Run full test suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: All pass, no warnings, formatted.

**Step 4: Commit**

```bash
git add src/tools/file.rs src/prompts/server_instructions.md
git commit -m "docs: update read_file description and server instructions for navigation params"
```

---

### Task 10: Final integration test and cleanup

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 2: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: Clean.

**Step 3: Manual smoke test**

Run: `cargo run -- start --project .`

Test with a real MCP call sequence:
1. `read_file("README.md")` — should show enriched heading summary with line ranges
2. `read_file("Cargo.toml")` — should show TOML table structure
3. `read_file("README.md", heading="## Development Commands")` — should return section content
4. `read_file("package.json", json_path="$.dependencies")` — should return deps subtree (if such file exists, or test with a fixture)

**Step 4: Squash/organize commits if needed, then final commit**

```bash
git log --oneline HEAD~8..HEAD  # Review commits
```
