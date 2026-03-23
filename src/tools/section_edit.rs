//! `edit_section` tool — heading-addressed Markdown section editing.

use anyhow::Result;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

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
    // We scan forward from body_start (heading_idx + 1) without skipping code fences —
    // a same-or-higher-level heading ends the section even if it's inside a fence.
    // This matches expected semantics: fenced headings at the same level close the section
    // boundary, preserving their content in `after` rather than treating it as part of the body.
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
                // Ensure a blank line between heading and body (standard Markdown convention).
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
            // Insert after end_idx (exclusive end of section).
            // Use join_lines (which always appends '\n') so the new content starts
            // on a fresh line immediately after the section's last line.
            let before = join_lines(&lines[..end_idx]);
            let after = join_lines_tail(&lines[end_idx..]);
            let result = format!("{}{}{}", before, new, after);
            Ok(normalize_trailing_newline(&result))
        }

        "remove" => {
            // Remove heading + body. Consume one trailing blank line if present.
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the exclusive-end index (into `split('\n')` lines) for a section
/// that starts at `start_idx` (0-based) and has heading level `level`.
///
/// Scans forward without skipping code-block headings — a same-or-higher-level
/// heading inside a fence still ends the section (this matches the expected
/// behavior for body replacement, where fenced headings at the same level should
/// close the section boundary).
fn compute_section_end(lines: &[&str], start_idx: usize, level: usize) -> usize {
    for (i, &line) in lines[start_idx..].iter().enumerate() {
        if let Some(lvl) = crate::tools::file_summary::heading_level(line) {
            if lvl <= level {
                return start_idx + i;
            }
        }
    }
    // Reached the end. If the last element is "" (trailing newline artifact from
    // split('\n')), include it as part of "after" when needed — return lines.len().
    // But for "end of section" we want the last real content line included, so
    // we return the index of the trailing "" element, which equals lines.len() - 1
    // when content ends with '\n', or lines.len() when it doesn't.
    // In all cases returning lines.len() is correct: lines[..lines.len()] is all lines.
    lines.len()
}

/// Join a non-tail slice of lines back into a string.
/// The slice is expected NOT to contain the trailing "" element (or it's a prefix slice).
/// Always appends a '\n' after the last element to act as a separator toward the next part.
/// Returns "" for an empty slice.
fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

/// Join a tail slice (including any trailing "" from split('\n')).
/// Restores the original string faithfully: if the slice ends with "", that represents
/// a trailing newline and we DON'T add an extra one.
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

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct EditSection;

#[async_trait::async_trait]
impl Tool for EditSection {
    fn name(&self) -> &str {
        "edit_section"
    }

    fn description(&self) -> &str {
        "Edit a document section by heading. Actions: replace, insert_before, insert_after, remove. Supports Markdown."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "heading": { "type": "string", "description": "Section heading to target, e.g. '## Auth'" },
                "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove"], "description": "Operation to perform" },
                "content": { "type": "string", "description": "New content. Required for replace/insert_before/insert_after." }
            },
            "required": ["path", "heading", "action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;
        let heading = super::require_str_param(&input, "heading")?;
        let action = super::require_str_param(&input, "action")?;
        let content = input["content"].as_str();

        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;

        if !path.ends_with(".md") && !path.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                "edit_section currently supports Markdown files only",
                "For TOML use edit_file with toml_key, for JSON use edit_file with json_path.",
            )
            .into());
        }

        let file_content = std::fs::read_to_string(&resolved)?;
        let new_content =
            perform_section_edit(&file_content, heading, action, content).map_err(|e| {
                RecoverableError::with_hint(e.to_string(), "Check heading name and action.")
            })?;
        std::fs::write(&resolved, &new_content)?;

        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }

        ctx.agent.reload_config_if_project_toml(&resolved).await;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent.mark_file_dirty(resolved.clone()).await;

        // Coverage hint: warn about unread sections (same pattern as edit_file).
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
        // Content starts with \n, so heading + \n + content = heading + \n + \n + body
        // which is exactly one blank line — not two.
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
}
