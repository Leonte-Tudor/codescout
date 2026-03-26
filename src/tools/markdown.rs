//! Markdown-specific tools: `read_markdown` and `edit_markdown`.
//!
//! `ReadMarkdown` provides heading-based navigation for `.md` files (heading map,
//! single-section, multi-section, and line-range reads).
//!
//! `EditMarkdown` provides heading-addressed section editing with support for
//! `action="edit"` (scoped string replacement) and batch mode.

use anyhow::Result;
use serde_json::{json, Value};

use super::{optional_u64_param, parse_bool_param, RecoverableError, Tool, ToolContext};
use crate::util::text::extract_lines;

// ── read_markdown ────────────────────────────────────────────────────────────

pub struct ReadMarkdown;

#[async_trait::async_trait]
impl Tool for ReadMarkdown {
    fn name(&self) -> &str {
        "read_markdown"
    }

    fn description(&self) -> &str {
        "Read a Markdown file with heading-based navigation. Returns heading map by default, \
         or targeted sections via heading/headings params."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Markdown file path relative to project root" },
                "heading": { "type": "string", "description": "Markdown section by heading (e.g. \"## Auth\")." },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of headings to read (returns multiple sections). Mutually exclusive with heading."
                },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        use super::output::{OutputGuard, OutputMode, OverflowInfo};

        let path = super::require_str_param(&input, "path")?;

        // Gate: .md files only
        if !path.ends_with(".md") && !path.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                "read_markdown only supports .md files",
                "Use read_file for non-markdown files.",
            )
            .into());
        }

        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_read_path(
            path,
            project_root.as_deref(),
            &security,
        )?;

        if resolved.is_dir() {
            return Err(RecoverableError::with_hint(
                format!("'{}' is a directory, not a file", path),
                "Use list_dir to browse directory contents, or provide a specific file path",
            )
            .into());
        }

        let text = std::fs::read_to_string(&resolved).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RecoverableError::with_hint(
                format!("file not found: '{}'", path),
                "Check the path with list_dir, or use find_file to locate the file",
            )
            .into(),
            _ => anyhow::anyhow!("failed to read {}: {}", resolved.display(), e),
        })?;

        // Extract params
        let heading = input["heading"].as_str();
        let headings_param = super::optional_array_param(&input, "headings");
        let start_line = optional_u64_param(&input, "start_line");
        let end_line = optional_u64_param(&input, "end_line");

        // Mutual exclusivity checks
        if heading.is_some() && headings_param.is_some() {
            return Err(RecoverableError::with_hint(
                "heading and headings are mutually exclusive",
                "Use heading for a single section, or headings for multiple sections.",
            )
            .into());
        }

        let has_nav = heading.is_some() || headings_param.is_some();
        let has_range = start_line.is_some() || end_line.is_some();

        if has_nav && has_range {
            return Err(RecoverableError::with_hint(
                "navigation parameters are mutually exclusive with start_line/end_line",
                "Use heading/headings OR start_line+end_line, not both",
            )
            .into());
        }

        if start_line.is_some() != end_line.is_some() {
            return Err(RecoverableError::with_hint(
                "both start_line and end_line are required",
                "Provide both start_line and end_line for a line range, e.g. start_line=1, end_line=50",
            )
            .into());
        }

        // ── Multi-heading navigation ─────────────────────────────────────
        if let Some(headings_arr) = headings_param {
            let heading_queries: Vec<String> = headings_arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            let mut sections = Vec::new();
            let mut seen_headings = Vec::new();

            for query in &heading_queries {
                let section = crate::tools::file_summary::extract_markdown_section(&text, query)?;
                seen_headings.push(
                    section
                        .breadcrumb
                        .last()
                        .cloned()
                        .unwrap_or_else(|| query.clone()),
                );
                sections.push(section.content);
            }

            let content = sections.join("\n\n");

            // Record coverage
            if !seen_headings.is_empty() {
                if let Ok(mut cov) = ctx.section_coverage.lock() {
                    cov.mark_seen(&resolved, &seen_headings);
                }
            }

            let mut result = json!({
                "content": content,
                "sections_returned": heading_queries.len(),
            });

            // Coverage hint
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
            if !all_headings.is_empty() {
                let all_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
                if let Ok(mut cov) = ctx.section_coverage.lock() {
                    if let Some(status) = cov.status(&resolved, &all_texts) {
                        if !status.unread.is_empty() {
                            result["coverage"] = json!({
                                "read": status.read_count,
                                "total": status.total_count,
                                "unread": status.unread,
                            });
                        }
                    }
                }
            }

            return Ok(result);
        }

        // ── Single-heading navigation ────────────────────────────────────
        if let Some(heading_query) = heading {
            let section_result =
                crate::tools::file_summary::extract_markdown_section(&text, heading_query)?;
            let cov = super::file::markdown_coverage(&text, &resolved, ctx, heading, None, None);

            // Buffer large sections
            if crate::tools::exceeds_inline_limit(&section_result.content) {
                let file_id = ctx.output_buffer.store_file(
                    resolved.to_string_lossy().to_string(),
                    section_result.content.clone(),
                );
                let mut val = json!({
                    "line_range": [section_result.line_range.0, section_result.line_range.1],
                    "breadcrumb": section_result.breadcrumb,
                    "siblings": section_result.siblings,
                    "format": "markdown",
                    "file_id": file_id,
                });
                if let Some(c) = cov {
                    val["coverage"] = c;
                }
                return Ok(val);
            }

            let mut val = json!({
                "content": section_result.content,
                "line_range": [section_result.line_range.0, section_result.line_range.1],
                "breadcrumb": section_result.breadcrumb,
                "siblings": section_result.siblings,
                "format": "markdown",
            });
            if let Some(c) = cov {
                val["coverage"] = c;
            }
            return Ok(val);
        }

        // ── Line-range read ──────────────────────────────────────────────
        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start == 0 || end < start {
                return Err(RecoverableError::with_hint(
                    format!(
                        "invalid line range: start_line={} end_line={} \
                         (start_line must be >= 1 and end_line >= start_line)",
                        start, end
                    ),
                    "Lines are 1-indexed. Example: start_line=1, end_line=50",
                )
                .into());
            }
            let content = extract_lines(&text, start as usize, end as usize);
            let md_cov =
                super::file::markdown_coverage(&text, &resolved, ctx, None, start_line, end_line);

            // Buffer large extracts
            if crate::tools::exceeds_inline_limit(&content) {
                let content_total = content.lines().count();
                let file_id = ctx
                    .output_buffer
                    .store_file(resolved.to_string_lossy().to_string(), content.clone());
                let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_budget(
                    &content,
                    1,
                    usize::MAX,
                    crate::tools::INLINE_BYTE_BUDGET,
                );
                let orig_start = start as usize;
                let orig_end = orig_start + lines_shown.saturating_sub(1);
                let mut result = json!({
                    "content": chunk,
                    "file_id": file_id,
                    "total_lines": content_total,
                    "shown_lines": [orig_start, orig_end],
                    "complete": complete,
                });
                if !complete {
                    let buf_next_start = lines_shown + 1;
                    let buf_next_end = (buf_next_start + lines_shown - 1).min(content_total);
                    result["next"] = json!(format!(
                        "read_markdown(\"{file_id}\", start_line={buf_next_start}, \
                         end_line={buf_next_end})"
                    ));
                }
                if let Some(c) = md_cov {
                    result["coverage"] = c;
                }
                return Ok(result);
            }

            let mut result = json!({ "content": content });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Default: heading map ─────────────────────────────────────────
        let total_lines = text.lines().count();

        // Buffer large files
        if crate::tools::exceeds_inline_limit(&text) {
            let file_id = ctx
                .output_buffer
                .store_file(resolved.to_string_lossy().to_string(), text.clone());
            let mut summary = crate::tools::file_summary::summarize_markdown(&text);
            summary["file_id"] = json!(file_id);
            if let Some(c) = super::file::markdown_coverage(&text, &resolved, ctx, None, None, None)
            {
                summary["coverage"] = c;
            }
            return Ok(summary);
        }

        // Small file: return content with exploring-mode cap
        let md_cov = super::file::markdown_coverage(&text, &resolved, ctx, None, None, None);

        let guard = OutputGuard::from_input(&input);
        let max_lines = guard.max_results;

        if guard.mode == OutputMode::Exploring && total_lines > max_lines {
            let content = extract_lines(&text, 1, max_lines);
            let overflow = OverflowInfo {
                shown: max_lines,
                total: total_lines,
                hint: format!(
                    "File has {} lines. Use start_line/end_line for ranges, \
                     or heading/headings for sections",
                    total_lines
                ),
                next_offset: None,
                by_file: None,
                by_file_overflow: 0,
            };
            let mut result = json!({ "content": content, "total_lines": total_lines });
            result["overflow"] = OutputGuard::overflow_json(&overflow);
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            Ok(result)
        } else {
            let mut result = json!({ "content": text, "total_lines": total_lines });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            Ok(result)
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(super::file::format_read_file(result))
    }
}

// ── edit_markdown ────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Helper functions (moved from section_edit.rs)
// ---------------------------------------------------------------------------

/// Pure string transformation: apply `action` to the section identified by `heading_query`.
///
/// Returns the full modified file content (always ends with a single newline).
pub fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String> {
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Split into lines using split('\n') so the trailing newline is preserved as
    // a final empty-string element: "a\nb\n".split('\n') == ["a", "b", ""].
    let lines: Vec<&str> = content.split('\n').collect();

    // Convert 1-based line numbers from the range to 0-based indices into `lines`.
    let heading_idx = (range.heading_line - 1) as usize;

    // Compute the exclusive-end index for the section.
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    match action {
        "replace" => {
            let new = new_content
                .ok_or_else(|| anyhow::anyhow!("content is required for the replace action"))?;

            // Smart detection: does the new content start with a Markdown heading?
            let replace_heading = new
                .lines()
                .next()
                .map(|l| heading_level(l.trim_end()).is_some())
                .unwrap_or(false);

            let result = if replace_heading {
                // Replace heading + body entirely.
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[end_idx..]);
                format!("{}{}{}", before, ensure_trailing_newline(new), after)
            } else {
                // Preserve the existing heading, replace body only.
                let heading_line_str = lines[heading_idx];
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[end_idx..]);
                let separator = if new.starts_with('\n') { "\n" } else { "\n\n" };
                format!(
                    "{}{}{}{}{}",
                    before,
                    heading_line_str,
                    separator,
                    ensure_trailing_newline(new),
                    after
                )
            };
            Ok(normalize_trailing_newline(&result))
        }

        "insert_before" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_before action")
            })?;
            let before = join_lines(&lines[..heading_idx]);
            let rest = join_lines_tail(&lines[heading_idx..]);
            let result = format!("{}{}{}", before, ensure_trailing_newline(new), rest);
            Ok(normalize_trailing_newline(&result))
        }

        "insert_after" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_after action")
            })?;
            let before = join_lines(&lines[..end_idx]);
            let after = join_lines_tail(&lines[end_idx..]);
            let result = format!("{}{}{}", before, new, after);
            Ok(normalize_trailing_newline(&result))
        }

        "remove" => {
            let mut remove_end = end_idx;
            if remove_end < lines.len() && lines[remove_end].trim().is_empty() {
                remove_end += 1;
            }
            let before = join_lines(&lines[..heading_idx]);
            let after = join_lines_tail(&lines[remove_end..]);
            let result = format!("{}{}", before, after);
            Ok(normalize_trailing_newline(&result))
        }

        other => Err(anyhow::anyhow!(
            "invalid action {:?}; expected replace, insert_before, insert_after, or remove",
            other
        )),
    }
}

/// Compute the exclusive-end index (into `split('\n')` lines) for a section
/// that starts at `start_idx` (0-based) and has heading level `level`.
fn compute_section_end(lines: &[&str], start_idx: usize, level: usize) -> usize {
    for (i, &line) in lines[start_idx..].iter().enumerate() {
        if let Some(lvl) = crate::tools::file_summary::heading_level(line) {
            if lvl <= level {
                return start_idx + i;
            }
        }
    }
    lines.len()
}

/// Join a non-tail slice of lines back into a string.
/// Always appends a '\n' after the last element to act as a separator.
fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

/// Join a tail slice (including any trailing "" from split('\n')).
fn join_lines_tail(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    lines.join("\n")
}

/// Ensure `s` ends with exactly one newline.
fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_owned()
    } else {
        format!("{}\n", s)
    }
}

/// Normalise the final result to end with exactly one newline.
fn normalize_trailing_newline(s: &str) -> String {
    let trimmed = s.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Perform a heading-scoped string replacement within a markdown file.
///
/// Finds the section identified by `heading_query`, locates `old_string` within it,
/// and replaces with `new_string`. If `replace_all` is true, replaces all occurrences
/// within the section; otherwise only the first.
///
/// Returns the full modified file content.
fn perform_scoped_edit(
    content: &str,
    heading_query: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String> {
    use crate::tools::file_summary::resolve_section_range;

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = (range.heading_line - 1) as usize;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    // Extract the section content (heading + body) with trailing newline
    let section_text = format!("{}\n", join_lines_tail(&lines[heading_idx..end_idx]));

    if !section_text.contains(old_string) {
        return Err(anyhow::anyhow!(
            "old_string not found in section '{}'. \
             The text must match exactly (whitespace-sensitive).",
            heading_query
        ));
    }

    let new_section = if replace_all {
        section_text.replace(old_string, new_string)
    } else {
        section_text.replacen(old_string, new_string, 1)
    };

    let before = join_lines(&lines[..heading_idx]);
    let after = join_lines_tail(&lines[end_idx..]);
    let result = format!("{}{}{}", before, new_section, after);
    Ok(normalize_trailing_newline(&result))
}

// ---------------------------------------------------------------------------
// EditMarkdown tool
// ---------------------------------------------------------------------------

pub struct EditMarkdown;

#[async_trait::async_trait]
impl Tool for EditMarkdown {
    fn name(&self) -> &str {
        "edit_markdown"
    }

    fn description(&self) -> &str {
        "Edit a Markdown document by heading. Actions: replace, insert_before, insert_after, \
         remove, edit. Supports batch mode via edits array."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Markdown file path" },
                "heading": { "type": "string", "description": "Target section heading (fuzzy matched)" },
                "action": {
                    "type": "string",
                    "enum": ["replace", "insert_before", "insert_after", "remove", "edit"],
                    "description": "Operation to perform"
                },
                "content": { "type": "string", "description": "New content for replace/insert actions (body only — heading preserved on replace)" },
                "old_string": { "type": "string", "description": "For action='edit': exact text to find within section" },
                "new_string": { "type": "string", "description": "For action='edit': replacement text" },
                "replace_all": { "type": "boolean", "default": false, "description": "For action='edit': replace all occurrences" },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["heading", "action"],
                        "properties": {
                            "heading": { "type": "string" },
                            "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove", "edit"] },
                            "content": { "type": "string" },
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" },
                            "replace_all": { "type": "boolean" }
                        }
                    },
                    "description": "Batch mode: array of edit operations applied atomically. Mutually exclusive with top-level heading/action."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;

        // Gate: .md files only
        if !path.ends_with(".md") && !path.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                "edit_markdown only supports .md files",
                "Use edit_file for non-markdown files.",
            )
            .into());
        }

        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;

        let file_content = std::fs::read_to_string(&resolved)?;

        // Determine mode: batch vs single
        let has_edits = input["edits"].is_array();
        let has_heading = input["heading"].is_string();
        let has_action = input["action"].is_string();

        if has_edits && (has_heading || has_action) {
            return Err(RecoverableError::with_hint(
                "edits array is mutually exclusive with top-level heading/action",
                "Use either edits=[] for batch mode, or heading+action for single edit.",
            )
            .into());
        }

        let new_content = if has_edits {
            // ── Batch mode ───────────────────────────────────────────
            let edits = input["edits"].as_array().unwrap();
            let mut content = file_content.clone();

            for (i, edit) in edits.iter().enumerate() {
                let heading = edit["heading"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'heading' field", i)
                })?;
                let action = edit["action"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'action' field", i)
                })?;

                content = if action == "edit" {
                    let old_string = edit["old_string"].as_str().ok_or_else(|| {
                        anyhow::anyhow!("edits[{}]: old_string is required for action='edit'", i)
                    })?;
                    let new_string = edit["new_string"].as_str().unwrap_or("");
                    let replace_all_val = edit["replace_all"].as_bool().unwrap_or(false);
                    perform_scoped_edit(&content, heading, old_string, new_string, replace_all_val)
                        .map_err(|e| {
                            RecoverableError::with_hint(
                                format!("edits[{}]: {}", i, e),
                                "Check heading name and old_string content.",
                            )
                        })?
                } else {
                    let edit_content = edit["content"].as_str();
                    perform_section_edit(&content, heading, action, edit_content).map_err(|e| {
                        RecoverableError::with_hint(
                            format!("edits[{}]: {}", i, e),
                            "Check heading name and action.",
                        )
                    })?
                };
            }

            content
        } else {
            // ── Single edit mode ─────────────────────────────────────
            let heading = super::require_str_param(&input, "heading")?;
            let action = super::require_str_param(&input, "action")?;

            if action == "edit" {
                let old_string = super::require_str_param(&input, "old_string")?;
                let new_string = input["new_string"].as_str().unwrap_or("");
                let replace_all_val = parse_bool_param(&input["replace_all"]);
                perform_scoped_edit(
                    &file_content,
                    heading,
                    old_string,
                    new_string,
                    replace_all_val,
                )
                .map_err(|e| {
                    RecoverableError::with_hint(e.to_string(), "Check heading name and old_string.")
                })?
            } else {
                let content = input["content"].as_str();
                perform_section_edit(&file_content, heading, action, content).map_err(|e| {
                    RecoverableError::with_hint(e.to_string(), "Check heading name and action.")
                })?
            }
        };

        crate::util::fs::atomic_write(&resolved, &new_content)?;

        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }

        ctx.agent.reload_config_if_project_toml(&resolved).await;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent.mark_file_dirty(resolved.clone()).await;

        // Coverage hint: warn about unread sections.
        let all_headings = crate::tools::file_summary::parse_all_headings(&new_content);
        if !all_headings.is_empty() {
            let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
            if let Ok(mut cov) = ctx.section_coverage.lock() {
                if let Some(hint) = cov.unread_hint(&resolved, &heading_texts) {
                    return Ok(json!({"status": "ok", "hint": hint}));
                }
            }
        }

        Ok(json!("ok"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── perform_section_edit tests (moved from section_edit.rs) ──────────

    #[test]
    fn replace_body_only() {
        let content = "# Title\n## Setup\nold content\nmore old\n## Usage\nuse it\n";
        let result =
            perform_section_edit(content, "## Setup", "replace", Some("new content\n")).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\n\nnew content\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn replace_with_heading() {
        let content = "# Title\n## Setup\nold content\n## Usage\nuse it\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "replace",
            Some("## Installation\nnew steps\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Installation\nnew steps\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn replace_empty_section() {
        let content = "# Title\n## Empty\n## Next\nstuff\n";
        let result =
            perform_section_edit(content, "## Empty", "replace", Some("now has content\n"))
                .unwrap();
        assert_eq!(
            result,
            "# Title\n## Empty\n\nnow has content\n## Next\nstuff\n"
        );
    }

    #[test]
    fn insert_before() {
        let content = "# Title\n## Setup\ncontent\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "insert_before",
            Some("## Prerequisites\ninstall stuff\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Prerequisites\ninstall stuff\n## Setup\ncontent\n"
        );
    }

    #[test]
    fn insert_after() {
        let content = "# Title\n## Setup\ncontent\n## Usage\nuse it\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "insert_after",
            Some("\n## Testing\ntest it\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\ncontent\n\n## Testing\ntest it\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn remove_section() {
        let content = "# Title\n## Setup\ncontent\n\n## Usage\nuse it\n";
        let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
        assert_eq!(result, "# Title\n## Usage\nuse it\n");
    }

    #[test]
    fn remove_last_section() {
        let content = "# Title\n## Setup\ncontent\n";
        let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
        assert_eq!(result, "# Title\n");
    }

    #[test]
    fn nested_section_replace() {
        let content =
            "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling\n";
        let result =
            perform_section_edit(content, "## Parent", "replace", Some("replaced all\n")).unwrap();
        assert_eq!(
            result,
            "# Title\n## Parent\n\nreplaced all\n## Sibling\nsibling\n"
        );
    }

    #[test]
    fn trailing_newline_normalization() {
        let content = "# Title\n## Setup\ncontent";
        let result = perform_section_edit(content, "## Setup", "replace", Some("new")).unwrap();
        assert!(
            result.ends_with('\n'),
            "result should end with newline: {:?}",
            result
        );
    }

    #[test]
    fn replace_body_preserves_blank_line_after_heading() {
        let content = "# Title\n\n## Goals\n\n- item 1\n- item 2\n\n## Next\n\nmore\n";
        let result =
            perform_section_edit(content, "Goals", "replace", Some("- new item\n")).unwrap();
        assert!(
            result.contains("## Goals\n\n- new item\n"),
            "should have blank line between heading and body: {:?}",
            result
        );
    }

    #[test]
    fn replace_body_no_double_blank_when_content_starts_with_newline() {
        let content = "# Title\n\n## Goals\n\n- item 1\n";
        let result =
            perform_section_edit(content, "Goals", "replace", Some("\n- new item\n")).unwrap();
        assert!(
            result.contains("## Goals\n\n- new item\n"),
            "should not produce double blank line: {:?}",
            result
        );
        assert!(
            !result.contains("## Goals\n\n\n"),
            "must not have triple newline: {:?}",
            result
        );
    }

    #[test]
    fn remove_only_section() {
        let content = "## Only\ncontent\n";
        let result = perform_section_edit(content, "## Only", "remove", None).unwrap();
        assert!(result.trim().is_empty() || result == "\n");
    }

    #[test]
    fn consecutive_edits() {
        let content = "# Title\n## A\noriginal a\n## B\noriginal b\n";
        let after_first =
            perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
        assert!(after_first.contains("updated a"));
        let after_second =
            perform_section_edit(&after_first, "## B", "replace", Some("updated b\n")).unwrap();
        assert!(after_second.contains("updated a"));
        assert!(after_second.contains("updated b"));
    }

    #[test]
    fn smart_replace_detection_non_heading() {
        let content = "# Title\n## Setup\nold content\n";
        let result =
            perform_section_edit(content, "## Setup", "replace", Some("#hashtag comment\n"))
                .unwrap();
        assert!(result.contains("## Setup"));
        assert!(result.contains("#hashtag comment"));
    }

    #[test]
    fn heading_inside_code_block_edit() {
        let content = "# Title\n## Real\ncontent\n```\n## Fake\n```\n";
        let result =
            perform_section_edit(content, "## Real", "replace", Some("new content\n")).unwrap();
        assert!(result.contains("## Real"));
        assert!(result.contains("new content"));
        assert!(result.contains("## Fake"));
    }

    #[test]
    fn duplicate_heading_edit_error() {
        let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond\n";
        let err = perform_section_edit(content, "## Example", "replace", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("found") && err.to_string().contains("times"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn heading_not_found() {
        let content = "# Title\n## Setup\ntext";
        let err =
            perform_section_edit(content, "## Nonexistent", "replace", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn missing_content_for_replace() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "replace", None).unwrap_err();
        assert!(
            err.to_string().contains("content"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn invalid_action() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "invalid", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("invalid"),
            "unexpected error: {}",
            err
        );
    }

    // ── perform_scoped_edit tests (action="edit") ────────────────────────

    #[test]
    fn scoped_edit_first_occurrence() {
        let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
        let result = perform_scoped_edit(content, "## Setup", "foo", "baz", false).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\nbaz bar foo\nmore foo\n## Next\nfoo\n"
        );
    }

    #[test]
    fn scoped_edit_replace_all() {
        let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
        let result = perform_scoped_edit(content, "## Setup", "foo", "baz", true).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\nbaz bar baz\nmore baz\n## Next\nfoo\n"
        );
    }

    #[test]
    fn scoped_edit_not_found() {
        let content = "# Title\n## Setup\ncontent\n";
        let err = perform_scoped_edit(content, "## Setup", "nonexistent", "x", false).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn scoped_edit_does_not_affect_other_sections() {
        let content = "# Title\n## A\nhello world\n## B\nhello world\n";
        let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
        assert!(result.contains("## A\ngoodbye world"));
        assert!(result.contains("## B\nhello world"));
    }

    #[test]
    fn scoped_edit_empty_replacement() {
        let content = "# Title\n## Setup\nremove this word\n";
        let result = perform_scoped_edit(content, "## Setup", " this", "", false).unwrap();
        assert_eq!(result, "# Title\n## Setup\nremove word\n");
    }

    // ── batch mode tests ────────────────────────────────────────────────

    #[test]
    fn batch_replace_two_sections() {
        let content = "# Title\n## A\nold a\n## B\nold b\n";
        let after_a = perform_section_edit(content, "## A", "replace", Some("new a\n")).unwrap();
        let after_b = perform_section_edit(&after_a, "## B", "replace", Some("new b\n")).unwrap();
        assert!(after_b.contains("new a"));
        assert!(after_b.contains("new b"));
    }

    #[test]
    fn batch_mixed_actions() {
        let content = "# Title\n## A\ncontent a\n## B\ncontent b\n## C\ncontent c\n";
        let step1 = perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
        let step2 = perform_section_edit(&step1, "## B", "remove", None).unwrap();
        let step3 = perform_section_edit(
            &step2,
            "## C",
            "insert_after",
            Some("\n## D\nnew section\n"),
        )
        .unwrap();
        assert!(step3.contains("updated a"));
        assert!(!step3.contains("## B"));
        assert!(step3.contains("## D\nnew section"));
    }

    #[test]
    fn batch_edit_action() {
        let content = "# Title\n## A\nhello world\n## B\nhello world\n";
        let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
        let result = perform_scoped_edit(&result, "## B", "hello", "hi", false).unwrap();
        assert!(result.contains("goodbye world"));
        assert!(result.contains("hi world"));
    }
}
