//! Text processing helpers.

/// Truncate a string to at most `max_chars` characters, appending `…` if cut.
pub fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars].iter().collect::<String>() + "…"
    }
}

/// Count lines in a string. An empty string has 0 lines.
pub fn count_lines(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    s.lines().count()
}

/// Extract a line range from text (1-indexed, inclusive). Returns empty string
/// if the range is out of bounds.
pub fn extract_lines(text: &str, start_line: usize, end_line: usize) -> String {
    text.lines()
        .enumerate()
        .filter(|(i, _)| {
            let line = i + 1;
            line >= start_line && line <= end_line
        })
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract lines from `start_line` to `end_line` (1-indexed, inclusive) without
/// exceeding `byte_budget` bytes. Returns `(content, lines_shown, complete)`.
///
/// - `content`: the extracted lines joined with `\n`
/// - `lines_shown`: number of lines included
/// - `complete`: true if all lines in the requested range were included
///
/// **Safety valve:** always includes at least 1 line (even if it exceeds the budget)
/// to prevent infinite retry loops where the agent keeps requesting the same range.
/// Exception: if byte_budget is 0, returns nothing (edge case for testing).
pub fn extract_lines_to_budget(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
) -> (String, usize, bool) {
    // Edge case: zero budget returns nothing
    if byte_budget == 0 {
        return ("".to_string(), 0, false);
    }

    let mut result_lines: Vec<&str> = Vec::new();
    let mut bytes_used: usize = 0;
    let mut hit_end = true; // assume complete unless budget breaks us out

    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if lineno < start_line {
            continue;
        }
        if lineno > end_line {
            break;
        }

        let line_bytes = line.len() + 1; // +1 for the \n join separator
        if bytes_used + line_bytes > byte_budget && !result_lines.is_empty() {
            hit_end = false;
            break;
        }

        result_lines.push(line);
        bytes_used += line_bytes;
    }

    let lines_shown = result_lines.len();
    (result_lines.join("\n"), lines_shown, hit_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_appends_ellipsis() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_unicode_counts_chars_not_bytes() {
        // "é" is 2 bytes but 1 char
        assert_eq!(truncate("héllo", 3), "hél…");
    }

    #[test]
    fn count_lines_empty() {
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn count_lines_single() {
        assert_eq!(count_lines("hello"), 1);
    }

    #[test]
    fn count_lines_multi() {
        assert_eq!(count_lines("a\nb\nc"), 3);
    }

    #[test]
    fn extract_lines_full_range() {
        assert_eq!(extract_lines("a\nb\nc", 1, 3), "a\nb\nc");
    }

    #[test]
    fn extract_lines_middle() {
        assert_eq!(extract_lines("a\nb\nc\nd\ne", 2, 4), "b\nc\nd");
    }

    #[test]
    fn extract_lines_single() {
        assert_eq!(extract_lines("a\nb\nc", 2, 2), "b");
    }

    #[test]
    fn extract_lines_out_of_bounds_returns_empty() {
        assert_eq!(extract_lines("a\nb", 10, 20), "");
    }

    #[test]
    fn extract_lines_first_line() {
        assert_eq!(extract_lines("first\nsecond\nthird", 1, 1), "first");
    }

    #[test]
    fn extract_lines_to_budget_fits_all() {
        let text = "short\nlines\nhere\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 10_000);
        assert_eq!(lines_shown, 3);
        assert!(complete);
        assert_eq!(content, "short\nlines\nhere");
    }

    #[test]
    fn extract_lines_to_budget_truncates_at_budget() {
        // Each line is 10 bytes ("line NNNN\n"). Budget of 25 bytes fits 2 full lines.
        let text: String = (1..=10).map(|i| format!("line {:04}\n", i)).collect();
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
        assert_eq!(lines_shown, 2);
        assert!(!complete);
        assert_eq!(content, "line 0001\nline 0002");
    }

    #[test]
    fn extract_lines_to_budget_respects_start_line() {
        let text = "aaa\nbbb\nccc\nddd\neee\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 3, 100, 10_000);
        assert_eq!(lines_shown, 3); // lines 3, 4, 5
        assert!(complete);
        assert_eq!(content, "ccc\nddd\neee");
    }

    #[test]
    fn extract_lines_to_budget_respects_end_line() {
        let text = "aaa\nbbb\nccc\nddd\neee\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 2, 4, 10_000);
        assert_eq!(lines_shown, 3); // lines 2, 3, 4
        assert!(complete); // all requested lines fit
        assert_eq!(content, "bbb\nccc\nddd");
    }

    #[test]
    fn extract_lines_to_budget_budget_hit_before_end_line() {
        // Request lines 1-100 but budget only fits ~2 lines
        let text: String = (1..=100).map(|i| format!("line {:04}\n", i)).collect();
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
        assert_eq!(lines_shown, 2);
        assert!(!complete);
        assert_eq!(content, "line 0001\nline 0002");
    }

    #[test]
    fn extract_lines_to_budget_zero_budget_returns_nothing() {
        let text = "aaa\nbbb\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 0);
        assert_eq!(lines_shown, 0);
        assert!(!complete);
        assert_eq!(content, "");
    }

    #[test]
    fn extract_lines_to_budget_single_line_exceeds_budget() {
        // A single very long line — must still return at least 1 line if budget > 0
        // to avoid infinite loops (agent would retry same range forever).
        let text = "a".repeat(1000);
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 1, 50);
        assert_eq!(lines_shown, 1);
        // complete = true because we reached end_line, even though it exceeded budget
        assert!(complete);
        assert_eq!(content.len(), 1000);
    }

    #[test]
    fn extract_lines_to_budget_empty_text() {
        let (content, lines_shown, complete) = extract_lines_to_budget("", 1, 100, 10_000);
        assert_eq!(lines_shown, 0);
        assert!(complete); // no lines to show, so "all" lines were shown
        assert_eq!(content, "");
    }

    #[test]
    fn extract_lines_to_budget_start_beyond_total() {
        let text = "aaa\nbbb\nccc\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 500, 600, 10_000);
        assert_eq!(lines_shown, 0);
        assert!(complete); // no lines in range, nothing to show
        assert_eq!(content, "");
    }
}
