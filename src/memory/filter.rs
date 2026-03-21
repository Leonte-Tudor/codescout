#[derive(Debug, PartialEq)]
pub struct FilterResult {
    /// Filtered markdown: preamble + matched section bodies.
    pub content: String,
    /// True if at least one requested section was found.
    pub matched: bool,
    /// Requested sections not found — preserves caller-supplied casing.
    pub missing: Vec<String>,
    /// All `### ` headings present in the file, normalized (trimmed), in file order.
    pub available: Vec<String>,
}

/// Filter markdown content to only the requested `### Heading` sections.
///
/// # Precondition
///
/// `sections` must be non-empty. Enforced via `debug_assert!` (fires in debug
/// builds / `cargo test`; compiled out in `--release`). The caller in
/// `Memory::call` checks `sections.is_empty()` before calling this function.
///
/// # Returns
///
/// Always returns a `FilterResult`. The caller checks `result.matched` to
/// decide whether to return content or a `RecoverableError`.
pub fn filter_sections(content: &str, sections: &[&str]) -> FilterResult {
    debug_assert!(!sections.is_empty(), "precondition: sections must be non-empty");
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Language Patterns

Intro line.

### Rust

Rust anti-patterns here.

#### Sub-heading

More Rust content.

### TypeScript

TypeScript patterns here.

### Python

Python patterns here.
";

    #[test]
    fn filter_sections_returns_matching_section() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"), "should include heading");
        assert!(r.content.contains("Rust anti-patterns here."), "should include body");
        assert!(r.content.contains("# Language Patterns"), "should include preamble");
        assert!(!r.content.contains("### TypeScript"), "should exclude TypeScript");
    }

    #[test]
    fn filter_sections_case_insensitive() {
        let r = filter_sections(SAMPLE, &["rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
    }

    #[test]
    fn filter_sections_multiple_sections() {
        let r = filter_sections(SAMPLE, &["Rust", "TypeScript"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        assert!(!r.content.contains("### Python"));
        assert!(r.missing.is_empty());
    }

    #[test]
    fn filter_sections_preserves_preamble() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.content.starts_with("# Language Patterns"));
    }

    #[test]
    fn filter_sections_no_match_returns_not_matched() {
        let r = filter_sections(SAMPLE, &["Go"]);
        assert!(!r.matched);
        assert_eq!(r.missing, vec!["Go"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }

    #[test]
    fn filter_sections_partial_match_returns_missing() {
        // "typescript" matches (case-insensitive); "Go" does not
        let r = filter_sections(SAMPLE, &["Rust", "typescript", "Go"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        // missing preserves caller-supplied casing
        assert_eq!(r.missing, vec!["Go"]);
    }

    #[test]
    fn filter_sections_duplicate_headings_both_included() {
        let content = "### Rust\n\nFirst block.\n\n### Rust\n\nSecond block.\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("First block."));
        assert!(r.content.contains("Second block."));
    }

    #[test]
    fn filter_sections_nested_h4_included_in_body() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.content.contains("#### Sub-heading"), "h4 should be part of section body");
        assert!(r.content.contains("More Rust content."));
    }

    #[test]
    fn filter_sections_heading_whitespace_normalized() {
        // Double space after ### and trailing space
        let content = "###  Rust  \n\nContent.\n";
        let r = filter_sections(content, &["rust"]);
        assert!(r.matched, "should match despite whitespace");
        assert_eq!(r.available, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_no_headings_in_file_returns_not_matched() {
        let content = "Just a preamble\nno headings here\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(!r.matched);
        assert!(r.available.is_empty());
        assert_eq!(r.missing, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_indented_heading_not_a_boundary() {
        // Leading space — NOT a section boundary
        let content = "### Real\n\nBody.\n\n ### Fake\n\nNot a section.\n";
        let r = filter_sections(content, &["Real"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Real"]);
        // The indented line is part of the "Real" section body
        assert!(r.content.contains(" ### Fake"));
    }

    #[test]
    #[should_panic(expected = "precondition")]
    fn filter_sections_empty_sections_is_caller_error() {
        // debug_assert! fires in debug builds (including `cargo test`).
        // This test will NOT catch the precondition violation in `--release` builds.
        filter_sections("### Rust\nContent\n", &[]);
    }

    #[test]
    fn filter_sections_available_in_file_order() {
        let r = filter_sections(SAMPLE, &["Python"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }
}
